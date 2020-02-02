.text
    .global mhartid
    mhartid:
        csrr a0, mhartid
        ret

    .global mvendorid
    mvendorid:
        csrr a0, mvendorid
        ret

    .global misa
    misa:
        csrr a0, misa
        ret

    .global ecall
    ecall:
        li t0, 0xcafebabe
        li t1, 0xdeadbeef
        ecall