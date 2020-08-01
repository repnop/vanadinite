use crate::{kernel_entry, utils::ptr::LinkerSymbol};

#[link_section = ".init.boot"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn _boot() -> ! {
    #[rustfmt::skip]
    asm!(r#"
        .equ PHYS_ADDR_MASK, 0x0FFF
        .equ PHYS_ADDR_SHIFT_DOWN, 12
        .equ PHYS_ADDR_SHIFT_UP, 10
        .equ VALID, 1
        .equ VALID_READ_EXECUTE, 0b1111
        .equ VPN_MASK, 0x1FF
        .equ VPN0_SHIFT, 12
        .equ VPN1_SHIFT, 21
        .equ VPN2_SHIFT, 30
        .equ PHYS_ADDR_TOP_MASK, 0x3FFFFFFFFFFFFF
        .equ TWO_MiB, 0x200000
        .equ SV39, 8 << 60
        .equ MMU_ON, (1 << 11) | (1 << 5) | (1 << 19)

        mv x19, a0
        mv x20, a1

        lla x1, __bss_start
        lla x2, __bss_end

        clear_bss_loop:
            bge x1, x2, clear_bss_end
            sd zero, (x1)
            addi x1, x1, 8
            j clear_bss_loop

        clear_bss_end:

        # x1 = kernel start virtual address
        # x2 = kernel end virtual address
        #
        # x3 = kernel start physical address
        # x4 = kernel stop physical address
        #
        # x5 = level one page table address
        # x6 = level two page table address
        
        # RISC-V uses PC-relative loading with code-model=medium
        # so we will never get the correct address by just loading
        # the symbol address, so use a "trick" to get the correct
        # virtual addresses
        #
        # We store the symbol address value in a DWORD in .data for
        # each symbol, then load that DWORD into the register
        #
        # This also means that we can just load the page table
        # symbols directly and don't have to translate them to
        # physical addresses from virtual
        lla t0, __kernel_virtual_start
        ld x1, (t0)

        lla t0, __kernel_virtual_end
        ld x2, (t0)

        lla x3, __kernel_start
        lla x4, __stack_top_virtual #__kernel_end

        # Mask the top bits for compatibility
        li x31, PHYS_ADDR_TOP_MASK

        lla x5, level_one_page_table
        and x5, x5, x31
        
        lla x6, level_two_page_table
        and x6, x6, x31

        # Extract VPN2 out of the start address
        mv x7, x1
        srli x7, x7, VPN2_SHIFT
        andi x7, x7, VPN_MASK

        # Multiply by 8 to get the byte offset
        # into the page table
        li x31, 8
        mul x7, x7, x31
        add x7, x5, x7

        # x7 now points inside of the level one
        # page table to the entry we want

        # Move the level two table address into
        # x31, and shift it into place, and set
        # it as a valid page table entry
        mv x31, x6
        srli x31, x31, PHYS_ADDR_SHIFT_DOWN
        slli x31, x31, PHYS_ADDR_SHIFT_UP
        ori x31, x31, VALID
        
        # Store the PTE at the offset into the
        # second page table
        sd x31, (x7)

        mv x10, x1

        # Start mapping the kernel in the second
        # level page table
        map_kernel_loop:
            bge x3, x4, map_kernel_done

            # Store the virtual address we're
            # mapping in x7, then get the VPN1
            # value out of it, and make it an
            # offset into the second page table
            li x31, 8
            mv x7, x10
            srli x7, x7, VPN1_SHIFT
            andi x7, x7, VPN_MASK
            mul x7, x7, x31
            add x7, x7, x6

            # Store the physical address into
            # x8, then prepare it as a PTE
            mv x8, x3
            srli x8, x8, PHYS_ADDR_SHIFT_DOWN
            slli x8, x8, PHYS_ADDR_SHIFT_UP
            ori x8, x8, VALID_READ_EXECUTE

            sd x8, (x7)

            li x31, TWO_MiB
            add x10, x10, x31
            add x3, x3, x31
            
            j map_kernel_loop

        map_kernel_done:
            # Put root level page table into SATP
            mv x31, x5
            srli x31, x31, PHYS_ADDR_SHIFT_DOWN
            li x30, SV39
            or x31, x31, x30
            csrw satp, x31

            # Switch MMU on
            li x31, MMU_ON
            csrw mstatus, x31

            # Load start address into MEPC
            lla x31, __start_virtual
            ld x30, (x31)

            csrw mepc, x30

            lla x31, __trap_shim_virtual
            ld x30, (x31)
            csrw mtvec, x30

            lla x31, __stack_top_virtual
            ld x30, (x31)
            mv sp, x30

            lla x31, __global_pointer_virtual$
            ld x30, (x31)
            # setup global pointer
            .option push
            .option norelax
            mv gp, x30
            .option pop

            mret

        .bss
        .align 12
        .globl level_one_page_table
        level_one_page_table:
            .rept 512
                .dword 0
            .endr
        .globl level_two_page_table
        level_two_page_table:
            .rept 512
                .dword 0
            .endr

        .data
        .align 3
        .globl __kernel_virtual_start
        __kernel_virtual_start: .dword __kernel_start
        .globl __kernel_virtual_end
        __kernel_virtual_end: .dword __kernel_end
        .globl __start_virtual
        __start_virtual: .dword _start
        .globl __stack_top_virtual
        __stack_top_virtual: .dword __stack_top
        .globl __global_pointer_virtual$
        __global_pointer_virtual$: .dword __global_pointer$
        .globl __trap_shim_virtual
        __trap_shim_virtual: .dword mtvec_trap_shim

        .section .init.boot
    "#);

    loop {}
}

#[link_section = ".init.rust"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() -> ! {
    let hart_id;
    let fdt;

    #[rustfmt::skip]
    asm!("
        nop
        nop
        nop
        nop
        mv {}, x19
        mv {}, x20
    ", out(reg) hart_id , out(reg) fdt);
    kernel_entry(hart_id, fdt);

    #[allow(unreachable_code)]
    asm!("1: wfi", "j 1b");

    loop {}
}
