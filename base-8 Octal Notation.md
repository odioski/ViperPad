# base-8 Octal Notation

`chmod xyz` uses three permission digits:

- `x` = owner/user
- `y` = group
- `z` = others

Each digit is a sum of:

- `4` = read
- `2` = write
- `1` = execute

That gives each position 8 possible values, from `0` to `7`:

| Value | Meaning |
| --- | --- |
| 0 | `---` |
| 1 | `--x` |
| 2 | `-w-` |
| 3 | `-wx` |
| 4 | `r--` |
| 5 | `r-x` |
| 6 | `rw-` |
| 7 | `rwx` |

With 3 digits total, there are **8 x 8 x 8 = 512** possible permission combinations.

## Example

`chmod 754 file`

- `7` for owner = `rwx`
- `5` for group = `r-x`
- `4` for others = `r--`
