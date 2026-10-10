# Metal CI command review

Both exact CI cargo check commands pass in the isolated devbox worktree with the pinned original fixture submodule available. The workflow adds two independent architecture jobs and always retains build logs. Mac framework linking, native GUI rendering and hardware execution are not covered.
