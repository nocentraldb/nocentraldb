---
type: internal
tickets: [NOC-331]
components: [sdk-rust]
---
The sync tests' bounds on a handshake and an answer are 5 seconds instead of 500 ms, so a relay that answers late under load is no longer taken for a lost one. A new test holds every message back by 400 ms each way and checks that the member stays in sync. Released code keeps its 30-second bounds.
