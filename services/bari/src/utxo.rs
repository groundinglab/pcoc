//! Confirmed plant-UTXO reserve.
//!
//! A seal spends one confirmed wallet output and leaves the others in place.
//! The change from that spend is unconfirmed, so the next seal must not need
//! it. The selector keeps a small confirmed pool (default 2–3) and rotates
//! through it. A skipped plant is not replaced, fee-bumped, or repaired.

use serde_json::Value;

/// Default minimum confirmations for a plant input. Zero is rejected:
/// a 0-conf change from the previous plant is not an input.
pub const DEFAULT_MIN_CONF: u64 = 1;
/// Low end of the confirmed pool operators keep for consecutive seals.
pub const DEFAULT_RESERVE_MIN: usize = 2;
/// High end of that pool. Consolidate, when the dial is on, folds the excess.
pub const DEFAULT_RESERVE_MAX: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtxoPolicy {
    pub min_conf: u64,
    pub reserve_min: usize,
    pub reserve_max: usize,
    /// `BARI_PLANT_UTXO_CONSOLIDATE=1`. Unset spends one input.
    pub consolidate: bool,
}

impl UtxoPolicy {
    pub fn standard() -> Self {
        Self {
            min_conf: DEFAULT_MIN_CONF,
            reserve_min: DEFAULT_RESERVE_MIN,
            reserve_max: DEFAULT_RESERVE_MAX,
            consolidate: false,
        }
    }

    /// `None` means the variable was unset and the default applies.
    pub fn from_parts(
        min_conf: Option<u64>,
        reserve_min: Option<u64>,
        reserve_max: Option<u64>,
        consolidate: Option<bool>,
    ) -> Result<Self, String> {
        let min_conf = min_conf.unwrap_or(DEFAULT_MIN_CONF);
        if min_conf < 1 {
            return Err(
                "BARI_PLANT_UTXO_MIN_CONF must be at least 1; a 0-conf change is not a plant input"
                    .to_owned(),
            );
        }
        let reserve_min = match reserve_min {
            None => DEFAULT_RESERVE_MIN,
            Some(value) => usize::try_from(value)
                .map_err(|_| "BARI_PLANT_UTXO_RESERVE_MIN does not fit".to_owned())?,
        };
        if reserve_min < DEFAULT_RESERVE_MIN {
            return Err(
                "BARI_PLANT_UTXO_RESERVE_MIN must be at least 2 (confirmed reserve defaults to 2-3)"
                    .to_owned(),
            );
        }
        let reserve_max = match reserve_max {
            None => DEFAULT_RESERVE_MAX.max(reserve_min),
            Some(value) => usize::try_from(value)
                .map_err(|_| "BARI_PLANT_UTXO_RESERVE_MAX does not fit".to_owned())?,
        };
        if reserve_max < reserve_min {
            return Err(
                "BARI_PLANT_UTXO_RESERVE_MAX must be greater than or equal to BARI_PLANT_UTXO_RESERVE_MIN"
                    .to_owned(),
            );
        }
        Ok(Self {
            min_conf,
            reserve_min,
            reserve_max,
            consolidate: consolidate.unwrap_or(false),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlantUtxo {
    pub txid: String,
    pub vout: u32,
    pub amount_sat: u64,
    pub confirmations: u64,
    pub spendable: bool,
    pub safe: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectedInput {
    pub txid: String,
    pub vout: u32,
    pub amount_sat: u64,
    pub confirmations: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectMode {
    RoundRobin,
    Consolidate,
}

impl SelectMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::Consolidate => "consolidate",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlantInputPlan {
    pub inputs: Vec<SelectedInput>,
    pub mode: SelectMode,
    pub confirmed: usize,
    pub thin: bool,
    pub next_cursor: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectError {
    NoConfirmed {
        min_conf: u64,
        confirmed: usize,
        reserve_min: usize,
        reserve_max: usize,
    },
    NoneAffordable {
        confirmed: usize,
        consolidate: bool,
    },
}

impl SelectError {
    /// The selector waits. The ops line says `deferred` and this reason.
    /// Nothing is broadcast and the waypoints stay in the pool.
    pub(crate) fn defer_message(self) -> String {
        match self {
            Self::NoConfirmed {
                min_conf,
                confirmed,
                reserve_min,
                reserve_max,
            } => format!(
                "no available confirmed plant UTXO (minconf={min_conf}, confirmed={confirmed}, reserve={reserve_min}-{reserve_max}); 0-conf outputs are not inputs; deferred; no tree; no psbt; not replaced"
            ),
            Self::NoneAffordable {
                confirmed,
                consolidate,
            } => format!(
                "no available confirmed plant UTXO can fund this plant (confirmed={confirmed}, consolidate={consolidate}); deferred; no tree; no psbt; not replaced"
            ),
        }
    }
}

/// Upper bound on the fee for a plant transaction.
///
/// Inputs are priced as legacy P2PKH (148 vB) so a witness input is
/// over-estimated. The plant still fails closed at funding time when the
/// bound was not enough; this function only decides which coins are candidates.
pub(crate) fn estimated_plant_fee_sat(input_count: usize, fee_rate_sat_vb: f64) -> u64 {
    const OVERHEAD_VB: f64 = 10.0;
    const INPUT_VB: f64 = 148.0;
    const OP_RETURN_VB: f64 = 51.0;
    const CHANGE_VB: f64 = 34.0;
    if input_count == 0 || !fee_rate_sat_vb.is_finite() || fee_rate_sat_vb <= 0.0 {
        return u64::MAX;
    }
    let vsize = OVERHEAD_VB + (INPUT_VB * input_count as f64) + OP_RETURN_VB + CHANGE_VB;
    let fee = vsize * fee_rate_sat_vb;
    if !fee.is_finite() || fee < 0.0 {
        return u64::MAX;
    }
    fee.ceil() as u64
}

pub(crate) fn select_plant_inputs(
    utxos: &[PlantUtxo],
    policy: &UtxoPolicy,
    fee_rate_sat_vb: f64,
    cursor: u64,
) -> Result<PlantInputPlan, SelectError> {
    let eligible = eligible_utxos(utxos, policy.min_conf);
    let confirmed = eligible.len();
    if confirmed == 0 {
        return Err(SelectError::NoConfirmed {
            min_conf: policy.min_conf,
            confirmed: 0,
            reserve_min: policy.reserve_min,
            reserve_max: policy.reserve_max,
        });
    }
    if policy.consolidate && confirmed > policy.reserve_max {
        if let Some(inputs) = consolidate_excess(&eligible, policy.reserve_max, fee_rate_sat_vb) {
            return Ok(plan(
                inputs,
                SelectMode::Consolidate,
                confirmed,
                policy,
                cursor,
            ));
        }
    }
    let single_fee = estimated_plant_fee_sat(1, fee_rate_sat_vb);
    let affordable: Vec<&PlantUtxo> = eligible
        .iter()
        .filter(|utxo| utxo.amount_sat >= single_fee)
        .collect();
    if affordable.is_empty() {
        if policy.consolidate {
            if let Some(inputs) = gather_for_fee(&eligible, fee_rate_sat_vb) {
                return Ok(plan(
                    inputs,
                    SelectMode::Consolidate,
                    confirmed,
                    policy,
                    cursor,
                ));
            }
        }
        return Err(SelectError::NoneAffordable {
            confirmed,
            consolidate: policy.consolidate,
        });
    }
    let index = (cursor % affordable.len() as u64) as usize;
    let chosen = selected(affordable[index]);
    Ok(plan(
        vec![chosen],
        SelectMode::RoundRobin,
        confirmed,
        policy,
        cursor,
    ))
}

pub(crate) fn parse_listunspent(value: &Value) -> Result<Vec<PlantUtxo>, String> {
    let rows = value
        .as_array()
        .ok_or("listunspent did not return an array")?;
    rows.iter().map(parse_unspent_row).collect()
}

fn plan(
    inputs: Vec<SelectedInput>,
    mode: SelectMode,
    confirmed: usize,
    policy: &UtxoPolicy,
    cursor: u64,
) -> PlantInputPlan {
    PlantInputPlan {
        inputs,
        mode,
        confirmed,
        thin: confirmed < policy.reserve_min,
        next_cursor: cursor.wrapping_add(1),
    }
}

fn eligible_utxos(utxos: &[PlantUtxo], min_conf: u64) -> Vec<PlantUtxo> {
    let mut eligible: Vec<PlantUtxo> = utxos
        .iter()
        .filter(|utxo| {
            utxo.spendable && utxo.safe && utxo.confirmations >= min_conf && utxo.amount_sat > 0
        })
        .cloned()
        .collect();
    eligible.sort_by(|left, right| left.txid.cmp(&right.txid).then(left.vout.cmp(&right.vout)));
    eligible
}

/// Spend the smallest outputs above the reserve and leave `reserve_max - 1`
/// confirmed outputs unspent. The change output is what restores the pool
/// after it confirms. Returns nothing when those excess outputs cannot pay.
fn consolidate_excess(
    eligible: &[PlantUtxo],
    reserve_max: usize,
    fee_rate_sat_vb: f64,
) -> Option<Vec<SelectedInput>> {
    if reserve_max == 0 || eligible.len() <= reserve_max {
        return None;
    }
    let leave = reserve_max - 1;
    let mut by_size = eligible.to_vec();
    by_size.sort_by(|left, right| {
        left.amount_sat
            .cmp(&right.amount_sat)
            .then(left.txid.cmp(&right.txid))
            .then(left.vout.cmp(&right.vout))
    });
    let spend_n = by_size.len() - leave;
    let spend = &by_size[..spend_n];
    let fee = estimated_plant_fee_sat(spend_n, fee_rate_sat_vb);
    let sum: u64 = spend
        .iter()
        .try_fold(0u64, |acc, utxo| acc.checked_add(utxo.amount_sat))?;
    if sum < fee {
        return None;
    }
    Some(spend.iter().map(selected).collect())
}

/// Fewest confirmed outputs that cover the fee. Largest first.
fn gather_for_fee(eligible: &[PlantUtxo], fee_rate_sat_vb: f64) -> Option<Vec<SelectedInput>> {
    let mut by_size = eligible.to_vec();
    by_size.sort_by(|left, right| {
        right
            .amount_sat
            .cmp(&left.amount_sat)
            .then(left.txid.cmp(&right.txid))
            .then(left.vout.cmp(&right.vout))
    });
    let mut sum: u64 = 0;
    for (index, utxo) in by_size.iter().enumerate() {
        sum = sum.checked_add(utxo.amount_sat)?;
        let count = index + 1;
        if sum >= estimated_plant_fee_sat(count, fee_rate_sat_vb) {
            return Some(by_size.iter().take(count).map(selected).collect());
        }
    }
    None
}

fn selected(utxo: &PlantUtxo) -> SelectedInput {
    SelectedInput {
        txid: utxo.txid.clone(),
        vout: utxo.vout,
        amount_sat: utxo.amount_sat,
        confirmations: utxo.confirmations,
    }
}

fn parse_unspent_row(row: &Value) -> Result<PlantUtxo, String> {
    let txid = row
        .get("txid")
        .and_then(Value::as_str)
        .ok_or("listunspent row is missing txid")?;
    if txid.len() != 64 || !txid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("listunspent txid must be 64 hexadecimal characters".to_owned());
    }
    let vout = row
        .get("vout")
        .and_then(Value::as_u64)
        .ok_or("listunspent row is missing vout")?;
    let vout = u32::try_from(vout).map_err(|_| "listunspent vout exceeds u32".to_owned())?;
    let amount = row
        .get("amount")
        .ok_or("listunspent row is missing amount")?;
    let amount_sat = btc_amount_to_sat(amount)?;
    let confirmations = row
        .get("confirmations")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let spendable = row.get("spendable").and_then(Value::as_bool) == Some(true);
    let safe = row.get("safe").and_then(Value::as_bool).unwrap_or(true);
    Ok(PlantUtxo {
        txid: txid.to_ascii_lowercase(),
        vout,
        amount_sat,
        confirmations,
        spendable,
        safe,
    })
}

fn btc_amount_to_sat(amount: &Value) -> Result<u64, String> {
    let btc = if let Some(number) = amount.as_f64() {
        number
    } else if let Some(text) = amount.as_str() {
        text.parse::<f64>()
            .map_err(|_| "listunspent amount is not numeric".to_owned())?
    } else {
        return Err("listunspent amount is not numeric".to_owned());
    };
    if !btc.is_finite() || btc < 0.0 {
        return Err("listunspent amount is not a finite non-negative BTC value".to_owned());
    }
    let sats = (btc * 100_000_000.0).round();
    if sats < 0.0 || sats > u64::MAX as f64 {
        return Err("listunspent amount is out of range".to_owned());
    }
    Ok(sats as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn coin(txid: &str, vout: u32, amount_sat: u64, confirmations: u64) -> PlantUtxo {
        PlantUtxo {
            txid: txid.to_owned(),
            vout,
            amount_sat,
            confirmations,
            spendable: true,
            safe: true,
        }
    }

    fn ids(plan: &PlantInputPlan) -> Vec<&str> {
        plan.inputs
            .iter()
            .map(|input| input.txid.as_str())
            .collect()
    }

    #[test]
    fn confirmed_utxo_is_preferred_over_larger_unconfirmed_change() {
        let change = coin("dd", 0, 5_000_000, 0);
        let confirmed = coin("aa", 1, 50_000, 3);
        let policy = UtxoPolicy::standard();
        let plan = select_plant_inputs(&[change, confirmed], &policy, 1.0, 0).unwrap();
        assert_eq!(plan.mode, SelectMode::RoundRobin);
        assert_eq!(ids(&plan), ["aa"]);
        assert_eq!(plan.inputs[0].confirmations, 3);
        assert!(plan.thin);
        assert_eq!(plan.confirmed, 1);
        assert_eq!(plan.inputs.len(), 1);
    }

    #[test]
    fn round_robin_rotates_confirmed_outputs_and_skips_change() {
        let wallet = vec![
            coin("cc", 0, 90_000, 9),
            coin("aa", 0, 50_000, 2),
            coin("bb", 1, 80_000, 4),
            coin("ee", 0, 9_000_000, 0),
        ];
        let policy = UtxoPolicy::standard();
        let mut cursor = 0;
        let mut seen = Vec::new();
        for _ in 0..4 {
            let plan = select_plant_inputs(&wallet, &policy, 1.0, cursor).unwrap();
            assert_eq!(plan.mode, SelectMode::RoundRobin);
            assert_eq!(plan.inputs.len(), 1);
            assert!(plan.inputs[0].confirmations >= 1);
            assert!(!plan.thin);
            assert_eq!(plan.confirmed, 3);
            seen.push(plan.inputs[0].txid.clone());
            cursor = plan.next_cursor;
        }
        assert_eq!(seen, ["aa", "bb", "cc", "aa"]);
    }

    #[test]
    fn only_unconfirmed_change_fee_skips_without_a_selection() {
        let change = coin("dd", 0, 5_000_000, 0);
        let error = select_plant_inputs(&[change], &UtxoPolicy::standard(), 1.0, 0).unwrap_err();
        let message = error.defer_message();
        assert!(
            message.contains("no available confirmed plant UTXO"),
            "{message}"
        );
        assert!(message.contains("deferred"), "{message}");
        assert!(message.contains("not replaced"), "{message}");
        assert!(message.contains("0-conf"), "{message}");
    }

    #[test]
    fn unsafe_and_unspendable_confirmed_outputs_are_not_inputs() {
        let mut unsafe_coin = coin("aa", 0, 80_000, 6);
        unsafe_coin.safe = false;
        let mut locked = coin("bb", 0, 80_000, 6);
        locked.spendable = false;
        let error = select_plant_inputs(&[unsafe_coin, locked], &UtxoPolicy::standard(), 1.0, 0)
            .unwrap_err();
        assert!(matches!(error, SelectError::NoConfirmed { .. }));
    }

    #[test]
    fn min_conf_above_one_leaves_a_single_confirmation_for_later() {
        let young = coin("aa", 0, 80_000, 1);
        let mature = coin("bb", 0, 80_000, 2);
        let mut policy = UtxoPolicy::standard();
        policy.min_conf = 2;
        let plan = select_plant_inputs(&[young, mature], &policy, 1.0, 0).unwrap();
        assert_eq!(ids(&plan), ["bb"]);
    }

    #[test]
    fn fragmented_wallet_stays_single_input_until_consolidate_is_on() {
        let wallet = vec![
            coin("aa", 0, 40_000, 2),
            coin("bb", 0, 50_000, 3),
            coin("cc", 0, 60_000, 4),
            coin("dd", 0, 70_000, 5),
            coin("ee", 0, 80_000, 6),
            coin("ff", 0, 10_000_000, 0),
        ];
        let off = select_plant_inputs(&wallet, &UtxoPolicy::standard(), 1.0, 0).unwrap();
        assert_eq!(off.mode, SelectMode::RoundRobin);
        assert_eq!(off.inputs.len(), 1);
        assert_eq!(ids(&off), ["aa"]);

        let mut on = UtxoPolicy::standard();
        on.consolidate = true;
        let folded = select_plant_inputs(&wallet, &on, 1.0, 0).unwrap();
        assert_eq!(folded.mode, SelectMode::Consolidate);
        // Five confirmed, reserve max 3 → leave 2, spend the three smallest.
        assert_eq!(ids(&folded), ["aa", "bb", "cc"]);
        assert!(folded.inputs.iter().all(|input| input.confirmations >= 1));
    }

    #[test]
    fn consolidate_inside_the_reserve_band_still_spends_one_input() {
        let wallet = vec![
            coin("aa", 0, 40_000, 2),
            coin("bb", 0, 50_000, 3),
            coin("cc", 0, 60_000, 4),
        ];
        let mut policy = UtxoPolicy::standard();
        policy.consolidate = true;
        let plan = select_plant_inputs(&wallet, &policy, 1.0, 1).unwrap();
        assert_eq!(plan.mode, SelectMode::RoundRobin);
        assert_eq!(ids(&plan), ["bb"]);
    }

    #[test]
    fn dusty_coins_fee_skip_unless_consolidate_can_fund_the_plant() {
        let dust = vec![
            coin("aa", 0, 200, 4),
            coin("bb", 0, 200, 4),
            coin("cc", 0, 200, 4),
            coin("dd", 0, 9_000_000, 0),
        ];
        let skipped = select_plant_inputs(&dust, &UtxoPolicy::standard(), 1.0, 0).unwrap_err();
        let message = skipped.defer_message();
        assert!(
            message.contains("no available confirmed plant UTXO"),
            "{message}"
        );
        assert!(message.contains("consolidate=false"), "{message}");

        let mut policy = UtxoPolicy::standard();
        policy.consolidate = true;
        let single = estimated_plant_fee_sat(1, 1.0);
        assert!(200 < single);
        let pair = estimated_plant_fee_sat(2, 1.0);
        assert!(400 >= pair, "two dust coins must cover the two-input bound");
        let plan = select_plant_inputs(&dust, &policy, 1.0, 0).unwrap();
        assert_eq!(plan.mode, SelectMode::Consolidate);
        assert_eq!(plan.inputs.len(), 2);
        assert!(plan.inputs.iter().all(|input| input.txid != "dd"));
    }

    #[test]
    fn reserve_dials_default_to_two_and_three_and_reject_zero_conf() {
        let standard = UtxoPolicy::from_parts(None, None, None, None).unwrap();
        assert_eq!(standard, UtxoPolicy::standard());
        assert_eq!(standard.reserve_min, 2);
        assert_eq!(standard.reserve_max, 3);
        assert!(!standard.consolidate);

        let band = UtxoPolicy::from_parts(Some(1), Some(2), Some(3), Some(false)).unwrap();
        assert_eq!(band, standard);

        let zero_conf = UtxoPolicy::from_parts(Some(0), None, None, None).unwrap_err();
        assert!(zero_conf.contains("0-conf"), "{zero_conf}");
        let thin_reserve = UtxoPolicy::from_parts(None, Some(1), None, None).unwrap_err();
        assert!(thin_reserve.contains("at least 2"), "{thin_reserve}");
        let inverted = UtxoPolicy::from_parts(None, Some(3), Some(2), None).unwrap_err();
        assert!(inverted.contains("RESERVE_MAX"), "{inverted}");
    }

    #[test]
    fn listunspent_rows_keep_confirmed_spendable_coins_and_a_zero_conf_change() {
        let parsed = parse_listunspent(&json!([
            {
                "txid": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                "vout": 1,
                "amount": 0.0005,
                "confirmations": 2,
                "spendable": true,
                "safe": true
            },
            {
                "txid": "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                "vout": 0,
                "amount": "0.01000000",
                "confirmations": 0,
                "spendable": true,
                "safe": true
            }
        ]))
        .unwrap();
        assert_eq!(parsed[0].amount_sat, 50_000);
        assert_eq!(parsed[0].confirmations, 2);
        assert_eq!(parsed[1].confirmations, 0);
        assert_eq!(parsed[1].amount_sat, 1_000_000);
        let plan = select_plant_inputs(&parsed, &UtxoPolicy::standard(), 1.0, 0).unwrap();
        assert_eq!(
            plan.inputs[0].txid,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(plan.inputs[0].vout, 1);
    }
}
