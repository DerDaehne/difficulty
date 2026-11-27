| language | model | task | pass@1 | pass@5 | rounds | tokens (in+cache/out) | codes |
|---|---|---|---|---|---|---|---|
| dif | haiku | calc | yes | yes | 1 | 11711/16315 | - |
| dif | haiku | cinema | yes | yes | 1 | 11984/11400 | - |
| dif | haiku | records | yes | yes | 1 | 11788/11578 | - |
| dif | haiku | vending | yes | yes | 1 | 11867/4555 | - |
| dif | haiku | wordcount | yes | yes | 1 | 11459/9949 | - |
| dif | sonnet | calc | yes | yes | 1 | 11690/2860 | - |
| dif | sonnet | cinema | yes | yes | 1 | 11964/2897 | - |
| dif | sonnet | records | no | yes | 2 | 24625/3872 | S1 x1 |
| dif | sonnet | vending | yes | yes | 1 | 11846/1623 | - |
| dif | sonnet | wordcount | no | yes | 2 | 23512/2119 | R4 x1 |

| language | model | pass@1 | pass@5 | mean rounds | tokens (in+cache/out) |
|---|---|---|---|---|---|
| dif | haiku | 5/5 | 5/5 | 1.0 | 58809/53797 |
| dif | sonnet | 3/5 | 5/5 | 1.4 | 83637/13371 |
