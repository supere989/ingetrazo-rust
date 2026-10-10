# Terminal presentation notification vs actual display acceptance

Actual ARM/Intel callback observer CI proves that a drawable can reach Completed, invoke addPresentedHandler, and report zero callback timestamp. Intel later drawables report positive timestamps even when FIFO head zero blocks the old policy. Original failures remain retained; layer geometry hypothesis was refuted by actual800×600 visible/non-occluded bounds.

Apple documents addPresentedHandler as immediately after presentation; presentedTime zero denotes unpresented or skipped. The callback is therefore a terminal presentation-service notification (also actual observed for skipped frames), distinct from GPU command completion. This policy requires BOTH actual GPU Completed and actual terminal callback before releasing the frame owner. It separately counts zero-time callbacks as skipped and positive-time callbacks as displayed. Callback absent, GPU pending/error or invalid time never grants safe retirement. Unknown complete-owner/window retention remains. Skipped frames never count toward visible presentation proof.

The unchanged15s WSI probe continues controlled redraws until at least two actual positive timestamps, drains their pending owners before resize, requires the exact requested native physical resize event, then requires four additional positive timestamps and pendingzero before PASS. Thus at least six actual displayed frames are mandatory; all-zero virtual presentation still fails. Submission counts and command completion cannot manufacture a passing display claim. Existing observer logs now distinguish submitted/retired/displayed/skipped separately.

Both Mac targets compile. Actual native execution of this policy remains pending. No deadline widening or default/native-host adoption. Physical Mac own-window pixels/full renderer remain open.

https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler(_:) ; https://developer.apple.com/documentation/metal/mtldrawable/presentedtime .
