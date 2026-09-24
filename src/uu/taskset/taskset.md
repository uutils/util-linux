# taskset

```
taskset [options] mask command [argument...]
taskset [options] -p [mask] pid
```

Set or retrieve a process's CPU affinity.

Without -p/--pid, sets the CPU affinity to mask and executes command.
With -p/--pid, gets or sets the affinity of an existing process by PID.
With -a/--all-tasks, operates on all threads of the given PID.
With -c/--cpu-list, mask is interpreted as a CPU list instead of a hex mask.

Mask formats (without -c/--cpu-list):
 - hex with optional prefix: 0xff, 0xFF, ff
 - comma-separated hex groups as in /proc/<pid>/status: 00000001,00000000

CPU list format (with -c/--cpu-list):
 - individual CPUs and ranges: 0, 0-3, 0,2-5
 - with stride: 0-6:2 (every 2nd CPU from 0 to 6)
