# UDP option requests

`UdpOptions()` is equivalent to `UdpOptions(address_reuse: false,
broadcast: false, dual_stack: false)`. The three-argument constructor also accepts
positional booleans. `address_reuse()`, `broadcast()` and `dual_stack()` return
the exact requested settings; all combinations are representable and independent.

The value is immutable. Repeated observations and constructing another option
set do not change the original. Equality includes all three flags.

Construction performs no socket I/O or platform probing. A future UdpBinding
must apply settings before bind and report unsupported requests at that boundary;
the options value neither silently drops them nor promises OS support. In
particular, the broadcast flag does not imply IPv6 broadcast support, and the
dual-stack flag does not select an address family on its own. The binding's
endpoint determines the family and its capability rules remain open.

TDD covers the default/explicit forms, all eight boolean combinations, named
arguments and identity of each independently changed flag. Actual option
application, binding and platform rejection require the separate UDP socket task.
