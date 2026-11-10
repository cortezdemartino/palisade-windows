# Manager U1 source integration

The maintained Manager source was available locally as `Palisade-Installer`, version
1.0.1, without Git metadata. This kit records the exact changed source, original
file hashes and shared module hashes so the integration is reviewable in a Windows
repository commit. It does not modify a published Manager executable or its pin.

Validate the maintained source:

```powershell
node scripts/integrate-manager.cjs C:\path\to\Palisade-Installer
```

Add `--apply` to apply the recorded source changes. The script checks the Manager
package identity, matching public trust anchor, every input hash and every target
file before writing. Unrecognized concurrent changes cause refusal. Reapplication
is safe. Shared validators and their tests are copied directly from this Windows
repository; application-specific changes are in `overrides`.

Run Manager's `npm run check` and package checks after integration. The current kit
does not authorize a Manager release. Use a new Manager release identity and
separate approval before replacing the existing pinned 1.0.1 executable.
