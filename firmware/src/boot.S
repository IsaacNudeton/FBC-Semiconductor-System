/*
 * FBC Firmware - ARM Cortex-A9 Startup Code
 *
 * This runs immediately after FSBL loads us into DDR.
 * Sets up stack, clears BSS, then jumps to Rust main.
 */

.section .text.boot, "ax"
.global _start
.type _start, %function
.align 4

_start:
    /* Disable interrupts */
    cpsid   if

    /* Set stack pointer (top of stack from linker script) */
    ldr     sp, =_stack_top

    /* Clear BSS section */
    ldr     r0, =_bss_start
    ldr     r1, =_bss_end
    mov     r2, #0
1:  /* clear_bss loop */
    cmp     r0, r1
    bge     2f
    str     r2, [r0], #4
    b       1b
2:  /* bss_done */

    /* Enable VFP/NEON if available (Cortex-A9 has it) */
    mrc     p15, 0, r0, c1, c0, 2   /* Read CPACR */
    orr     r0, r0, #(0xF << 20)    /* Enable CP10 and CP11 */
    mcr     p15, 0, r0, c1, c0, 2   /* Write CPACR */
    isb

    /* Enable FPU */
    mov     r0, #(1 << 30)          /* Set EN bit in FPEXC */
    vmsr    fpexc, r0

    /* Jump to Rust main (never returns) */
    bl      main

    /* Should never reach here, but loop if we do */
3:  /* hang */
    wfi
    b       3b

.size _start, . - _start
