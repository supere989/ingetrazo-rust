Corrected UI alpha retains a strict residual

Actual NVIDIA own-window production UI after 2039629+2afa5bd passes 22/24 strict QPainter comparisons. The solid RGB (37,93,157), alpha128 over white yields native(145,173,206) versus QPainter(146,174,206), in both baseline and partial captures. All managed checker alpha/tint/partial controls now match. Gate remains failed. The prior 17-failure baseline is intact.

First attempt failed due to a fixture state JSON read during truncate/write. No acceptance was recorded; the reader now retries incomplete JSON and the complete fresh retry is retained. Source correction itself was unchanged. No general UI AA, hardware platform, lifecycle, or world parity claim.
