# Inter and Fraunces, served from this origin

| File | Font | CSS |
|---|---|---|
| `Inter-latin.woff2` | Inter (variable, weight 100 to 900) | `Inter`, used by `home.css` |
| `Fraunces-latin.woff2` | Fraunces (variable, weight 100 to 900, optical size 9 to 144) | `Fraunces`, the headings and the large figures in `home.css` |

Inter is copyright 2020 The Inter Project Authors (https://github.com/rsms/inter), under the SIL Open Font License 1.1 (`OFL-Inter.txt`).
`Inter-latin.woff2` is Inter 4.0's variable font, cut to the Latin ranges and the layout features the page uses (`pyftsubset` from fontTools, no other change), then compressed to woff2.

Fraunces is copyright 2018 The Fraunces Project Authors (https://github.com/undercasetype/Fraunces), under the SIL Open Font License 1.1 (`OFL-Fraunces.txt`).
`Fraunces-latin.woff2` is the upright variable font from that repository (`fonts/variable`, commit `7ccdec3`), with its Softness and Wonky axes fixed at 0 (`fonttools varLib.instancer`), cut to the Latin ranges (`pyftsubset` from fontTools, no other change), then compressed to woff2.
