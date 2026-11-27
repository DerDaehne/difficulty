| language | model | task | pass@1 | pass@5 | rounds | tokens (in+cache/out) | codes |
|---|---|---|---|---|---|---|---|
| dif | haiku | calc | yes | yes | 1 | 10560/17426 | - |
| dif | haiku | cinema | yes | yes | 1 | 10835/9009 | - |
| dif | haiku | records | yes | yes | 1 | 10637/16125 | - |
| dif | haiku | vending | no | yes | 2 | 22282/9160 | - |
| dif | haiku | wordcount | yes | yes | 1 | 10309/4395 | - |
| dif | sonnet | calc | yes | yes | 1 | 10541/2998 | - |
| dif | sonnet | cinema | yes | yes | 1 | 10815/2970 | - |
| dif | sonnet | records | yes | yes | 1 | 10617/3141 | - |
| dif | sonnet | vending | no | yes | 2 | 22295/3200 | - |
| dif | sonnet | wordcount | yes | yes | 1 | 10288/1306 | - |
| python | haiku | calc | yes | yes | 1 | 1192/2961 | - |
| python | haiku | cinema | yes | yes | 1 | 1467/2611 | - |
| python | haiku | records | yes | yes | 1 | 1270/3223 | - |
| python | haiku | vending | no | yes | 4 | 8162/12719 | - |
| python | haiku | wordcount | yes | yes | 1 | 941/380 | - |
| python | sonnet | calc | yes | yes | 1 | 1172/1666 | - |
| python | sonnet | cinema | yes | yes | 1 | 1446/1420 | - |
| python | sonnet | records | yes | yes | 1 | 1250/1485 | - |
| python | sonnet | vending | yes | yes | 1 | 1309/1038 | - |
| python | sonnet | wordcount | yes | yes | 1 | 919/274 | - |

| language | model | pass@1 | pass@5 | mean rounds | tokens (in+cache/out) |
|---|---|---|---|---|---|
| dif | haiku | 4/5 | 5/5 | 1.2 | 64623/56115 |
| dif | sonnet | 4/5 | 5/5 | 1.2 | 64556/13615 |
| python | haiku | 4/5 | 5/5 | 1.6 | 13032/21894 |
| python | sonnet | 5/5 | 5/5 | 1.0 | 6096/5883 |
