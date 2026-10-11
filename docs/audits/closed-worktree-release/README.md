# Completed worktree release

Three clean terminal checkout copies were removed after verifying each retained branch still names its exact HEAD, its committed tree exists, its only working change is the expected original-source symlink, and no live process cwd is inside it. The symlink was replaced by an empty submodule placeholder before ordinary git worktree remove; no force removal was used. All three branches and their tracked source/evidence remain in Git. Active worktrees, original Python source, current proof input directories and reusable targets remain.

cleanup.json records 1,418,168,045 apparent bytes removed. This is a filesystem apparent-size total, not a measured device-free-space delta. A historical checkout can be recreated with git worktree add PATH BRANCH using its preserved branch in that record. Earlier audit commands naming those temporary directories refer to their recorded historical checkouts.
