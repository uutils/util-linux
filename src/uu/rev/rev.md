# rev

```
rev [options] [file ...]
```

Reverse lines characterwise.

A record reverses by code point when it is valid UTF-8 and the first nonempty locale setting in LC_ALL, LC_CTYPE, LANG order specifies UTF-8; otherwise the whole record reverses bytewise.
