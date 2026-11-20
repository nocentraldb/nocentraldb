# 2026-11-12 · NOC-273 · Accounts get six new notices: DeviceAdded, DevicePaired, AgentPaired, AccountLinked, AccountRecovered and AccountKeyRenewed

- Decided by: Carsten (2026-11-12, on NOC-273, question 8, option A)
- Why: the texts, as proposed. `DeviceAdded`, once per space: "Your new device was added to this space and reads its whole history." `DevicePaired`: "This device now holds your account key. Keep your recovery code: it is the way back if you lose every device." `AgentPaired`: "This agent acts for your account in the spaces an admin adds it to. It holds no account key." `AccountLinked`: "Spaces this database joined before keep its old account. Other members see it as linked to yours." `AccountRecovered`: "Your account is back with a new account key. Your spaces come back when one of their members adds this device." `AccountKeyRenewed`: "Your account has a new account key. Members that have not learnt it yet still accept the old one."
- Instead of: reusing `MemberAdded` for the automatic adds (B), whose text is about someone else, not the person's own device.
- Topics: accounts, notices, texts, rfc-0009
