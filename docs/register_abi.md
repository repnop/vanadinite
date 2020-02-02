| Register | ABI Name | Description                      | Saver  |
|----------|----------|----------------------------------|--------|
| x0       | zero     | Hard-wired zero                  | --     |
| x1       | ra       | Return address                   | Caller |
| x2       | sp       | Stack pointer                    | Callee |
| x3       | gp       | Global pinter                    | --     |
| x4       | tp       | Thread pointer                   | --     |
| x5       | t0       | Temp/Alt. link register          | Caller |
| x6-7     | t1-2     | Temporaries                      | Caller |
| x8       | s0/fp    | Saved register/frame pointer     | Callee |
| x9       | s1       | Saved register                   | Callee |
| x10-11   | a0-1     | Function arguments/return values | Caller |
| x12-17   | a2-7     | Function arguments               | Caller |
| x18-27   | s2-11    | Saved registers                  | Callee |
| x28-31   | t3-6     | Temporaries                      | Caller |