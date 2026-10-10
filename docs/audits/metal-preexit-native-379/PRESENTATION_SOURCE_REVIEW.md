# Presentation scheduling source review

The pinned winit 0.30.13 macOS `pre_present_notify` implementation is an empty method (platform_impl/macos/window_delegate.rs:921). Adding it cannot change this backend’s actual notification behavior. The current source retains the callback block before `command.presentDrawable` and `commit`; callback observation stores timestamp then publishes its seen flag using release ordering, and owner polling acquires that flag. Unknown callbacks retain owners.

Apple’s documented default asynchronous CAMetalLayer path uses presentsWithTransaction=false and command-buffer presentation. Current actual snapshots report false, matching that path. The transaction=true variant requires a different ordering; changing the flag alone while retaining command-buffer presentation would violate the documented contract.

Authoritative references:
- https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction
- https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler(_:) (selector addPresentedHandler:)
- https://developer.apple.com/documentation/metal/mtldrawable/presentedtime

Actual visible content, positive GPU execution times, and command Completed do not substitute for a drawable’s positive presentation timestamp. Apple documents timestamp zero for either not-yet-presented or dropped frames; zero alone does not prove a terminal drop. Current unknown callbacks therefore remain unresolved. CAMetalDisplayLink is an official alternate native display cadence, but no evidence yet proves that adopting it resolves this Paravirtual condition. No source correction is claimed by this audit.
