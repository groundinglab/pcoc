//! Produce for the Windows core.
//!
//! A finished run always returns object bytes. A simulated Bari waypoint
//! POST failure is recorded and does not stop the walk. The walk uses the
//! scripted rig (`pcoc_produce::produce_on_rig`) so the harness can finish
//! without gossip. Waypoint posts are attempted after each stage that has
//! a next waypoint, matching the live producer's "enqueue and continue".

use crate::history::{self, Prefs};
use crate::pin::{OBJECT_KIND, SPEC, STAGES};
use pcoc_produce::bari::PostRequest;
use pcoc_produce::{encode_produced, produce_on_rig, ProduceOutcome, Progress, ScriptedRig};
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

/// One recorded waypoint POST. `error` is set when the post did not succeed.
/// The produce run still continues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostRecord {
    pub stage: usize,
    pub waypoint_hex: String,
    pub error: Option<String>,
}

/// What a waypoint POST did. The producer does not treat `Err` as fatal.
pub trait WaypointPost {
    fn post(&self, request: &PostRequest) -> Result<(), String>;
}

/// A hub that fails every post. The error string is recorded.
pub struct FailingWaypointPost {
    pub message: String,
}

impl WaypointPost for FailingWaypointPost {
    fn post(&self, _request: &PostRequest) -> Result<(), String> {
        Err(self.message.clone())
    }
}

/// A hub that accepts every post.
pub struct AcceptingWaypointPost;

impl WaypointPost for AcceptingWaypointPost {
    fn post(&self, _request: &PostRequest) -> Result<(), String> {
        Ok(())
    }
}

/// Outcome of one produce. `posts` may contain errors. `object` is still set
/// when the walk finished. `saved` is the cleartext `.cbor` path.
#[derive(Clone, Debug)]
pub struct ProduceResult {
    pub object_kind: String,
    pub spec: &'static str,
    pub stages: usize,
    pub posts: Vec<PostRecord>,
    /// Stages closed after a recorded post error. Equals the stage count
    /// when every post failed and the walk still finished.
    pub stages_closed_after_post_errors: usize,
    pub saved: PathBuf,
    pub bytes: Vec<u8>,
}

/// Run the scripted rig, post each waypoint as the walk passes it, and always
/// save the object.
///
/// A post error is appended to `posts` and the walk continues. The function
/// returns `Err` only when the walk itself was cancelled or the `.cbor`
/// could not be written. A Bari miss is not that error.
pub fn produce_and_save(
    public_key: &[u8; 32],
    rig: &ScriptedRig,
    profile: &pcoc::profile::Profile,
    hub: &dyn WaypointPost,
    history: &Path,
) -> Result<ProduceResult, String> {
    let cancel = AtomicBool::new(false);
    let posts = Mutex::new(Vec::new());
    let continued = AtomicUsize::new(0);
    // The scripted rig's first leg of a stage checkpoints the stage limb,
    // and that limb is the stage waypoint (see `pcoc_produce::produce_on_rig`:
    // `previous = stage_limb`, then one checkpoint, and `waypoint: stage_limb`).
    // With one leg per stage the leg event carries that waypoint's limb via
    // the checkpoint we recompute is unnecessary: the object stores it, and
    // the leg event's stage tells us when to post the previous stage's
    // waypoint. We keep the latest leg's stage so the interrupt can post.
    let latest_leg_stage = Mutex::new(None::<usize>);
    let hub_ref = hub;
    let posts_ref = &posts;
    let continued_ref = &continued;
    let latest_ref = &latest_leg_stage;
    let outcome = produce_on_rig(public_key, profile, rig, &cancel, |event| {
        match event {
            Progress::Leg { stage, .. } => {
                *latest_ref.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(stage);
            }
            Progress::Interrupt { stage, height } => {
                // Live produce enqueues the next waypoint when a stage
                // closes, and does not enqueue after the last stage.
                // A failed POST is recorded. This callback returns, and
                // `cancel` stays clear, so the walk continues.
                if stage + 1 >= STAGES {
                    return;
                }
                let seen = latest_ref.lock().unwrap_or_else(|poison| poison.into_inner());
                debug_assert_eq!(*seen, Some(stage));
                let request = PostRequest {
                    stage: stage + 1,
                    waypoint: waypoint_label(stage + 1, public_key),
                    beacon_height: height,
                };
                let error = match hub_ref.post(&request) {
                    Ok(()) => None,
                    Err(message) => Some(message),
                };
                if error.is_some() {
                    continued_ref.fetch_add(1, Ordering::SeqCst);
                }
                posts_ref
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .push(PostRecord {
                        stage: stage + 1,
                        waypoint_hex: hex_of(&request.waypoint),
                        error,
                    });
            }
            Progress::TerrainFilled | Progress::Closed => {}
        }
    });
    let object = match outcome {
        ProduceOutcome::Object(object) => object,
        ProduceOutcome::Cancelled => return Err("produce cancelled".to_owned()),
    };
    // The POST during the walk used a stage label so the failure is on the
    // continuing path before the object exists. The saved record names the
    // waypoint the object stored for that stage.
    let mut recorded = posts.into_inner().unwrap_or_else(|poison| poison.into_inner());
    for post in &mut recorded {
        if let Some(stage) = object.stages.get(post.stage) {
            post.waypoint_hex = hex_of(&stage.waypoint);
        }
    }
    let bytes = encode_produced(&object);
    debug_assert_eq!(object.kind, OBJECT_KIND);
    debug_assert!(!cancel.load(Ordering::SeqCst));
    let saved = history::save_object(history, &hex_of(public_key), &bytes)
        .map_err(|err| format!("could not write the object: {err}"))?;
    Ok(ProduceResult {
        object_kind: object.kind,
        spec: SPEC,
        stages: object.stages.len(),
        posts: recorded,
        stages_closed_after_post_errors: continued.load(Ordering::SeqCst),
        saved,
        bytes,
    })
}

fn hex_of(bytes: &[u8]) -> String {
    crate::hex::encode(bytes)
}

/// Stable label for a waypoint POST that fires before the object is sealed.
/// The saved [`PostRecord`] replaces this with the object's waypoint hex.
fn waypoint_label(stage: usize, public_key: &[u8; 32]) -> [u8; 32] {
    let mut label = *public_key;
    label[31] = stage as u8;
    label
}

/// Default prefs the README documents. No secret field.
pub fn default_prefs() -> Prefs {
    Prefs::default()
}

/// Stage count the produce path is pinned to.
pub fn pinned_stages() -> usize {
    STAGES
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcoc::profile::Profile;
    use pcoc_produce::{cases_profile, cases_rig, Case};

    fn tiny_profile() -> Profile {
        Profile::conformance_small(32, 64, 65_536, 1).expect("profile")
    }

    #[test]
    fn bari_post_failure_is_recorded_and_the_object_is_still_saved() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-windows-produce-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let key = [0x11u8; 32];
        // The scripted S1 rig is the fixture schedule. The profile is the
        // smallest conformance profile so the harness finishes quickly.
        // Waypoint bytes still come from the walk; the POST is simulated.
        let rig = cases_rig(Case::S1);
        let _ = cases_profile(Case::S1);
        let hub = FailingWaypointPost {
            message: "waypoint POST failed: connection refused".to_owned(),
        };
        let result = produce_and_save(&key, &rig, &tiny_profile(), &hub, &dir)
            .expect("produce continues after Bari miss");
        assert_eq!(result.stages, 18);
        assert_eq!(result.object_kind, "v2.2-pcoc-object");
        assert_eq!(result.spec, "2.3");
        assert_eq!(result.posts.len(), 17);
        assert_eq!(result.stages_closed_after_post_errors, 17);
        assert!(result.posts.iter().all(|post| post.error.is_some()));
        assert!(result
            .posts
            .iter()
            .all(|post| post.error.as_deref().unwrap().contains("POST failed")));
        assert!(result.posts.iter().all(|post| post.waypoint_hex.len() == 64));
        assert!(crate::history::is_cbor_file(&result.saved));
        assert_eq!(std::fs::read(&result.saved).unwrap(), result.bytes);
        assert!(!result.bytes.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn accepting_posts_still_save_and_record_no_error() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-windows-produce-ok-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let key = [0x22u8; 32];
        let rig = cases_rig(Case::S1);
        let result =
            produce_and_save(&key, &rig, &tiny_profile(), &AcceptingWaypointPost, &dir).unwrap();
        assert!(result.posts.iter().all(|post| post.error.is_none()));
        assert_eq!(result.posts.len(), 17);
        assert_eq!(result.stages_closed_after_post_errors, 0);
        assert!(result.saved.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
