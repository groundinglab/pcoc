# admission-claims

One process on the Vivobook issues the groundinglab admission badge. Stage 1 is `POST /badge` and `GET /health`. The name table is created and unused. Lookups of the published files never reach this process.

The award `r` tag cites `https://admission.groundinglab.io/log/<accept day>.jsonl#<accept seq>`.

strfry write policy is already deployed. The versioned copies are `deploy/strfry-allowlist-v2.py` and `deploy/issuer.txt`. Kinds 30009, 8, and 1742 are accepted only from that issuer pubkey. Kind 1808 is not in that list.

`deploy/blocked-names.txt` is the starting block list for names. The live copy is `/etc/admission-claims/blocked-names.txt`, re-read on every name request. Blocked attempts log `blocked name attempt` (view: `journalctl -u admission-claims | grep "blocked name"`).

Install notes are in [deploy/DEPLOY.md](deploy/DEPLOY.md). An empty `r2_account_id` is directory mode: artifacts stay in `artifact_dir`.
