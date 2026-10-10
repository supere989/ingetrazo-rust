Restricted warm pose handles

Root review586683eb exposed the previous Drop-based guard loophole: safe Rust mem::forget could bypass UUID-change invalidation. That previous bounded stage is not reliable against unrestricted identity edits.

ImagePoseGuard now has immutable Deref and only set_origin/set_frame mutation; no DerefMut or mutable UUID/whole-object access. No destructor is required for membership reliability. Two actual compiler controls reject UUID assignment and whole-object replacement through the handle. The50000-image probe explicitly sets origin then forgets the restricted handle: identity/epoch stay correct; varying warm1000live and1000detached hovers still scan0 and allocate0. Unrestricted IndexMut/poses_mut/iter_mut invalidate before exposure, so forgetting them cannot skip invalidation.

Fresh source36/mixed105/pure65 remain exact; oldfour/five failures and strict mutations remain required. Fullworkspace passes including both compile-fail controls. GPU reference drawing and other transform scopes remain open.
