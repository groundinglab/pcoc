## Appendix A. Every disagreeing key (values per build; `—` = key not emitted)

### rate.txt (45 keys)
All 45 keys disagree on spelling only. Decision (pass/fail) agrees on 45/45. Spellings: Rust `meets the floor`/`refuse`; C, Go, B2 `meet`/`refuse`; Java `match`/`mismatch`; B1 `meets`/`refuse`. Pass side on 17 keys, fail side on 28 in every build.

### draw.txt (2 keys)
| key | rust | c | go | java | b1 | b2 |
|---|---|---|---|---|---|---|
| `draw.legs.18.six_leg` | `7,2,1,15,10,16` | `17,2,11,7,13,3` | `7,2,1,15,10,16` | `7,2,1,15,10,16` | `7,2,1,15,10,16` | `7,2,1,15,10,16` |
| `draw.legs.54.six_leg` | `7,20,1,51,10,16` | `31,39,32,40,30,29` | `7,20,1,51,10,16` | `7,20,1,51,10,16` | `7,20,1,51,10,16` | `7,20,1,51,10,16` |

### hashcash.txt (10 keys)
| key | rust | c | go | java | b1 | b2 |
|---|---|---|---|---|---|---|
| `hashcash.challenge` | `4284847f89790951f5eca6a6d3f7052bf986ecb9597383638bbb47adcb375353` | `6714d0dd1bde62a94584d8942c415aafe5e4e486b6531612c2c4c3a239269e97` | `d63e8345aadedd05091b1cece6c3349827e3440056d880e8ea980f6bcdceda02` | `b563ee85308ff1346f6bb4c6d7bfb7c7a97381d71cb8e83d4dac41456a111d25` | `d1d65714b910b3c8c4122434b2ddee01113bce6152b4b1a7bcf74aa07d4115fa` | `58e405b889824c9ff3444c6e11aea358cc5e216e6fce57d794cb1ef2d3bc4f37` |
| `hashcash.digest` | `000006c7527cda01397e27bfb2fcfc5abd2c456eadfdfdf26831047c7074c8a2` | `0000020a1a86d896416fcad8d943a88d3f3bf32d4c8728135f13553954c2d3c4` | `0000027ac15a7fcb6354f3579a3926e4c85916a5848a651edda9454c5c1760e6` | `0000025566b3a98c831ceb648117b24500ce5fe7bffb41f831af0cd6eb299528` | `0000038b74f5f1fd6994ac122b33d322e10a39810660400272679808ce553725` | `000002afa0802db0c0dc9a5b1879f9c0311efd883c6d25a686136db6831f3411` |
| `hashcash.digest_nonce0` | — | `583a276149cc3166e29017505ce80ce7ee154dd557801bb5434c150993174df6` | `2555ac625fb7acd64321d72e39017cd92b9cdda7b83a53675527dd88bcda8b22` | — | — | — |
| `hashcash.fee_header_label` | — | `pcoc-pack-2.2/hashcash.fee-header` | `pcoc-pack-2.2/hashcash.fee_header` | — | — | — |
| `hashcash.header_hash` | `2a607ce2d0675d6462344a407bd901b9b24fbf501e4e6ee03125b7efc2e645d8` | — | — | `cb39c6f7e332dc799aa1d15ba4d92432239f2836278c138f75d58a2c0ff015e6` | `9e48d971c25a2c21c95449623e42b620c9037038afb020d12f4126309e849d54` | `287c28c72e4a7d23d88e82afbf601cd8740345018abc6ae9a78eec996c44f2a7` |
| `hashcash.nonce` | `1073395` | `65820` | `414931` | `3082081` | `1548251` | `4447969` |
| `hashcash.origin` | `https://bari.example` | — | — | `https://bari.example` | `https://4a0e0526df729135d43fac93d9548835.invalid` | `https://example.com` |
| `hashcash.origin_label` | `https://bari.example` | `https://bari.example` | `https://bari.example` | — | `bari_origin` | `bari-origin` |
| `hashcash.waypoint` | `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` | — | — | `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` | `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` | `6863fc06e217f9a102ee6f5be8761bc27425c0f0f4c6b722ca03aa4e5fdbd771` |
| `hashcash.waypoint_label` | — | `pcoc-pack-2.2/hashcash.waypoint` | `pcoc-pack-2.2/hashcash.waypoint` | — | `hashcash/waypoint` | — |

### negatives.txt (8 keys)
| key | rust | c | go | java | b1 | b2 |
|---|---|---|---|---|---|---|
| `negatives.all4_kernel.value` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `f9f9aa9a08688c0fe721e01a1502b542a56d9c7e03e37b43f8f9351c6de125b5` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `af0b0f2228b6505d58417d075653d61c6489ef9f23a5c5257d5d549d284926de` |
| `negatives.cached_leg.value` | `1cf2e2a5fc0849a3` | `4d9a6a98574cbd31` | `0265f616e37b5fbe` | `1cf2e2a5fc0849a3` | `3b715e2a251a44d9` | `4c9a6a98574cbd31` |
| `negatives.f3_shared_terrain.value` | `9dcd7e927f15553e1ddd757d5bfa902dc58f787910e2f62b5d4479da4606078c` | `366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f` | `366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f` | `9dcd7e927f15553e1ddd757d5bfa902dc58f787910e2f62b5d4479da4606078c` | `a73019b8a2c43cd1` | `3f5d4812c0ad1307d215633817f03cfab9a69bc6a8549e86e08c64846cf112fb` |
| `negatives.one_lane_flip.value` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `282cc1b65d4d89b15bfc3d7c5e073d08f521dc7c0400244589c08bd516c7bd91` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `a0325421bbc38a5ee9f494e957298684084086ccefb7821d3e8b9785923d2a45` |
| `negatives.partial_slot_read.value` | `738755dabac824ab` | `059f134920349f54` | `e1ae647ee5a5d7c5` | `e1ae647ee5a5d7c5` | `059f134920349f54` | `e1ae647ee5a5d7c5` |
| `negatives.partial_slot_read_2047.value` | — | `738755dabac824ab` | `738755dabac824ab` | `738755dabac824ab` | `738755dabac824ab` | `4c9a6a98574cbd31` |
| `negatives.prefold_hoist.value` | `a652562037b465fc` | `72fdcef7ceb359ec` | `b1808c8d3927a7d1` | `251da3dcd0d2c738` | `b1808c8d3927a7d1` | `73a4c7fbd105aae1` |
| `negatives.squeezed_seed_terrain.value` | `e343932c4f2705ce637acdd45a156a0fa7ece8a318323fb67e4034e6386d6bd5` | `ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22` | `794883d4ccc0bfe612bf830f48be3c3e38f01f9c158f4cc2c93dcf63e6dddf5c` | `e343932c4f2705ce637acdd45a156a0fa7ece8a318323fb67e4034e6386d6bd5` | `ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22` | `8f5162aa56660daf391411b219fe4f8141f43a4a58ac131f5ab762809244f04d` |

## Appendix B. Keys emitted by only some builds, all values agreeing

- **nl4** (24): `nl4.slot.0.word.10.base` (rust, java, b1); `nl4.slot.0.word.11.base` (rust, java, b1); `nl4.slot.0.word.12.base` (rust, java, b1); `nl4.slot.0.word.13.base` (rust, java, b1); `nl4.slot.0.word.14.base` (rust, java, b1); `nl4.slot.0.word.15.base` (rust, java, b1); `nl4.slot.0.word.8.base` (rust, java, b1); `nl4.slot.0.word.9.base` (rust, java, b1); `nl4.slot.1.word.10.base` (rust, java, b1); `nl4.slot.1.word.11.base` (rust, java, b1); `nl4.slot.1.word.12.base` (rust, java, b1); `nl4.slot.1.word.13.base` (rust, java, b1); `nl4.slot.1.word.14.base` (rust, java, b1); `nl4.slot.1.word.15.base` (rust, java, b1); `nl4.slot.1.word.8.base` (rust, java, b1); `nl4.slot.1.word.9.base` (rust, java, b1); `nl4.slot.1023.word.10.base` (rust, java, b1); `nl4.slot.1023.word.11.base` (rust, java, b1); `nl4.slot.1023.word.12.base` (rust, java, b1); `nl4.slot.1023.word.13.base` (rust, java, b1); `nl4.slot.1023.word.14.base` (rust, java, b1); `nl4.slot.1023.word.15.base` (rust, java, b1); `nl4.slot.1023.word.8.base` (rust, java, b1); `nl4.slot.1023.word.9.base` (rust, java, b1)
- **fold** (1): `fold.leg_result_u64#dup:fold.S1.pk_A.stage0.leg0.legs_le_hex` (go, java)
- **draw** (5): `draw.b0_label` (b1); `draw.legs.18.thorough` (b1); `draw.legs.54.thorough` (b1); `draw.legs.6.thorough` (b1); `draw.thorough_rounds` (b1)
- **hashcash** (7): `hashcash.bits` (c); `hashcash.difficulty` (rust, b1); `hashcash.fee_header` (b1); `hashcash.fee_header_hash_name` (java); `hashcash.fee_header_labels` (b1); `hashcash.header_hash_name` (rust); `hashcash.waypoint_name` (rust, java)
- **negatives** (3): `negatives.PROD.partial_slot_read_2047.matches_honest` (rust); `negatives.PROD.partial_slot_read_2047.value` (rust); `negatives.partial_slot_read_2047.matches_honest` (c, go, java, b1, b2)
