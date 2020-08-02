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
        .equ READ, 0b0010
        .equ WRITE, 0b0100
        .equ EXECUTE, 0b1000
        .equ VPN_MASK, 0x1FF
        .equ VPN0_SHIFT, 12
        .equ VPN1_SHIFT, 21
        .equ VPN2_SHIFT, 30
        .equ PHYS_ADDR_TOP_MASK, 0x3FFFFFFFFFFFFF
        .equ TWO_MiB, 0x200000
        .equ SV39, 8 << 60
        .equ MMU_ON, (1 << 11) | (1 << 5) | (1 << 19)
        .equ LOOK_AT_ME_IM_THE_CAPTAIN_NOW, 0b00011111

        li t0, LOOK_AT_ME_IM_THE_CAPTAIN_NOW
        csrw pmpcfg0, t0

        li t0, -1
        csrw pmpaddr0, t0

        sfence.vma x0, x0

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

        lla t0, level_one_page_table
        li t1, 512
        clear_pt1:
            beqz t1, clear_pt1_done
            sd zero, (t0)
            addi t1, t1, -1
            addi t0, t0, 8
            j clear_pt1

        clear_pt1_done:

        lla t0, level_two_page_table
        li t1, 512
        clear_pt2:
            beqz t1, clear_pt2_done
            sd zero, (t0)
            addi t1, t1, -1
            addi t0, t0, 8
            j clear_pt2

        clear_pt2_done:

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
        ld a0, (t0)

        lla t6, __kernel_end
        lla a1, __kernel_start
        
        map_kernel:
            bge a1, t6, map_kernel_done
            li a2, READ | EXECUTE | VALID
            call map_region

            li t0, TWO_MiB
            add a0, a0, t0
            add a1, a1, t0

            j map_kernel
            
        map_kernel_done:

        lla a0, __stack_bottom_virtual
        ld a0, (a0)

        lla a1, __stack_bottom
        lla t6, __stack_top

        map_stack:
            bge a1, t6, map_stack_done
            li a2, READ | WRITE | VALID
            call map_region

            li t0, TWO_MiB
            add a0, a0, t0
            add a1, a1, t0

            j map_stack

        map_stack_done:
    
        # Put root level page table into SATP
        lla t0, level_one_page_table
        srli t0, t0, PHYS_ADDR_SHIFT_DOWN
        li t1, SV39
        or t0, t0, t1
        csrw satp, t0

        # Switch MMU on
        li t0, MMU_ON
        csrw mstatus, t0

        # Load start address into MEPC
        lla t0, __start_virtual
        ld t1, (t0)

        csrw mepc, t1

        lla t0, __trap_shim_virtual
        ld t1, (t0)
        csrw mtvec, t1

        lla t0, __stack_top_virtual
        ld t1, (t0)
        mv sp, t1

        lla t0, __global_pointer_virtual$
        ld t1, (t0)
        # setup global pointer
        .option push
        .option norelax
        mv gp, t1
        .option pop

        sfence.vma

        mret

        # Map a single 2-MiB aligned region to a
        # virtual address
        #
        # Parameters:
        #   a0 = virtual address start
        #   a1 = physical address start
        #   a2 = permissions
        map_region:
            .equ TWO_MiB_ALIGN_MASK, 0x1FFFFF
            mv t0, a0
            mv t1, a1

            li t2, TWO_MiB_ALIGN_MASK
            
            and t0, t0, t2
            bnez t0, bunk_address

            and t1, t1, t2
            bnez t1, bunk_address
            
            # Mask the top bits for compatibility
            li t0, PHYS_ADDR_TOP_MASK

            lla s1, level_one_page_table
            and s1, s1, t0
            
            lla s2, level_two_page_table
            and s2, s2, t0

            # Extract VPN2 out of the start address
            mv s3, a0
            srli s3, s3, VPN2_SHIFT
            andi s3, s3, VPN_MASK

            # Multiply by 8 to get the byte offset
            # into the page table
            li t0, 8
            mul s3, s3, t0
            add s3, s1, s3

            # s3 now points inside of the level one
            # page table to the entry we want

            # Move the level two table address into
            # s4, and shift it into place, and set
            # it as a valid page table entry
            mv s4, s2
            srli s4, s4, PHYS_ADDR_SHIFT_DOWN
            slli s4, s4, PHYS_ADDR_SHIFT_UP
            ori s4, s4, VALID

            # Store the PTE at the offset into the
            # second page table
            sd s4, (s3)

            # Store the virtual address we're
            # mapping in s5, then get the VPN1
            # value out of it, and make it an
            # offset into the second page table
            li t0, 8
            mv s5, a0
            srli s5, s5, VPN1_SHIFT
            andi s5, s5, VPN_MASK
            mul s5, s5, t0
            add s5, s5, s2

            # Store the physical address into
            # s6, then prepare it as a PTE
            mv s6, a1
            srli s6, s6, PHYS_ADDR_SHIFT_DOWN
            slli s6, s6, PHYS_ADDR_SHIFT_UP
            or s6, s6, a2

            # Make sure this page wasn't already mapped
            ld t0, (s5)
            andi t0, t0, 1
            bnez t0, already_mapped

            sd s6, (s5)

            ret

        bunk_address:
            wfi
            j bunk_address

        already_mapped:
            wfi
            j already_mapped

        .bss
        .p2align 12
        .globl level_one_page_table
        level_one_page_table:
            .rept 512
                .dword 0
            .endr
        .p2align 12
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
        .globl __stack_bottom_virtual
        __stack_bottom_virtual: .dword __stack_bottom
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
pub unsafe extern "C" fn _start() -> ! {
    let hart_id;
    let fdt;

    #[rustfmt::skip]
    asm!("
        .align 8
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
