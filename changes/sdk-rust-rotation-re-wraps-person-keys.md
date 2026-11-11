---
type: internal
tickets: [NOC-206]
components: [sdk-rust]
---
A rotation of the Rust SDK, on a removal, a re-grant or the add of a former member, re-wraps every person key of the space under the new generation and puts the signed records, which replace the old wraps at the relay; a space with a person key the device does not hold yet is still refused.
