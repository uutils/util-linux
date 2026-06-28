# pivot_root

```
pivot_root NEW_ROOT PUT_OLD
```

Change the root filesystem.

Moves the root filesystem of the calling process to the directory PUT_OLD and
makes NEW_ROOT the new root filesystem.

This command requires the CAP_SYS_ADMIN capability and is typically used during
container initialization or system boot.

- NEW_ROOT must be a mount point
- PUT_OLD must be at or underneath NEW_ROOT