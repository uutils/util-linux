
// List of directories, man, src, and binary

pub const MANDIRS: [&str; 7] = [ 
	"/usr/man/*",
	"/usr/share/man/*",
	"/usr/X386/man/*",
	"/usr/X11/man/*",
	"/usr/TeX/man/*",
	"/usr/interviews/man/mann",
	"/usr/share/info",
	// NULL
];

pub const SRCDIRS: [&str; 6] = [
	"/usr/src/*",
	"/usr/src/lib/libc/*",
	"/usr/src/lib/libc/net/*",
	"/usr/src/ucb/pascal",
	"/usr/src/ucb/pascal/utilities",
	"/usr/src/undoc",
	// NULL
];


pub const BINDIRS: [&str; 46] = [
	"/usr/bin",
	"/usr/sbin",
	"/bin",
	"/sbin",

// #[cfg(...)]
/* 
#if defined(MULTIARCHTRIPLET)

	"/lib/" MULTIARCHTRIPLET,
	"/usr/lib/" MULTIARCHTRIPLET,
	"/usr/local/lib/" MULTIARCHTRIPLET,

#endif
*/

// #[cfg(not(...))]

	"/usr/lib",
	"/usr/lib32",
	"/usr/lib64",
	"/etc",
	"/usr/etc",
	"/lib",
	"/lib32",
	"/lib64",
	"/usr/games",
	"/usr/games/bin",
	"/usr/games/lib",
	"/usr/emacs/etc",
	"/usr/lib/emacs/*/etc",
	"/usr/TeX/bin",
	"/usr/tex/bin",
	"/usr/interviews/bin/LINUX",

	"/usr/X11R6/bin",
	"/usr/X386/bin",
	"/usr/bin/X11",
	"/usr/X11/bin",
	"/usr/X11R5/bin",

	"/usr/local/bin",
	"/usr/local/sbin",
	"/usr/local/etc",
	"/usr/local/lib",
	"/usr/local/games",
	"/usr/local/games/bin",
	"/usr/local/emacs/etc",
	"/usr/local/TeX/bin",
	"/usr/local/tex/bin",
	"/usr/local/bin/X11",

	"/usr/contrib",
	"/usr/hosts",
	"/usr/include",

	"/usr/g++-include",

	"/usr/ucb",
	"/usr/old",
	"/usr/new",
	"/usr/local",
	"/usr/libexec",
	"/usr/share",

	"/opt/*/bin",
	// NULL
];

