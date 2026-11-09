# Parent-loss observer startup correction

Windows checkpoint `e4e53322a5d281612219d7bf285f35e2f1cbd129` failed
[CI 42676519053](https://github.com/cortezdemartino/palisade-windows/actions/runs/42676519053)
in the exact-handle parent-loss process test. Its native unit tests passed, but
later CI packaging and application checks were skipped. Do not count that run as
passing.

The observer's `GetProcessById` failed before handle retention: the event-only
holder had already exited. That probe deliberately has a six-second failure
timeout. The failing case took about seven seconds, consistent with cold
PowerShell startup outlasting the probe. This is different from the older
post-parent-death survivor timeout documented in the parent-loss report.

The read-only observer now completes PowerShell startup before the disposable
fixture parent launches its three native processes. It then accepts their IDs
over its private stdin, validates the image, retains the original process handles
and creation times, and acknowledges retention before the parent may exit.
Cold startup, retention setup and final output have separately bounded harness
timers. Independent source review identified and corrected an initially shared
timer that could otherwise have reintroduced cold-start failures.

The product code and native six-second probe timeout are unchanged. Both clean
and forced fixture-parent death still require all three original processes to
exit within the unchanged four-second shared deadline, with monitor/coordinator
codes 125 and holder code 0. No PID disappearance fallback, retry, ignored failure
or widened native cleanup deadline was added. Only the observer and known
disposable fixture parent have harness termination paths.

The focused parent-loss case and all 25 no-input process groups passed locally.
The final timer adjustment is verified again before commit. These are real
Windows processes with no input or resource authority; they do not establish
physical-input cleanup, live Assist interoperability or device acceptance.
No installer execution, production service use, activation or release occurred.
