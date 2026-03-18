/*
 * FBC Firmware - ARM Cortex-A9 Startup Code
 *
 * This runs immediately after FSBL loads us into DDR.
 * Sets up vector table, stack, clears BSS, then jumps to Rust main.
 */

/* ======================================================================
 * Exception Vector Table (must be 32-byte aligned)
 * ====================================================================== */
.section .vectors, "ax"
.global _vector_table
.align 5

_vector_table:
    b       _start              /* 0x00: Reset */
    b       _hang               /* 0x04: Undefined Instruction */
    b       _hang               /* 0x08: Software Interrupt (SVC) */
    b       _hang               /* 0x0C: Prefetch Abort */
    b       _hang               /* 0x10: Data Abort */
    nop                         /* 0x14: Reserved */
    b       _irq_handler        /* 0x18: IRQ */
    b       _hang               /* 0x1C: FIQ */

/* ======================================================================
 * Startup Code
 * ====================================================================== */
.section .text.boot, "ax"
.global _start
.type _start, %function
.align 4

_start:
    /* Disable interrupts during init */
    cpsid   if

    /* Set VBAR to our vector table */
    ldr     r0, =_vector_table
    mcr     p15, 0, r0, c12, c0, 0  /* Write VBAR */
    isb

    /* Set up SVC mode stack (main execution mode) */
    ldr     sp, =_stack_top

    /* Set up IRQ mode stack */
    cps     #0x12               /* Switch to IRQ mode */
    ldr     sp, =_irq_stack_top /* IRQ stack (separate from main) */
    cps     #0x13               /* Back to SVC mode */

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

    /* Enable I-cache for performance (no MMU needed).
     * Without I-cache, every instruction fetch goes to DDR (~100ns),
     * making the CPU ~100x slower than expected. */
    mov     r0, #0
    mcr     p15, 0, r0, c7, c5, 0   /* ICIALLU: invalidate entire I-cache */
    dsb
    isb
    mrc     p15, 0, r0, c1, c0, 0   /* Read SCTLR */
    orr     r0, r0, #(1 << 12)      /* Set I bit (I-cache enable) */
    bic     r0, r0, #(1 << 0)       /* Ensure MMU stays off */
    mcr     p15, 0, r0, c1, c0, 0   /* Write SCTLR */
    isb

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
_hang:
    wfi
    b       _hang

.size _start, . - _start

/* ======================================================================
 * IRQ Handler — saves context, calls Rust dispatcher, restores
 * ====================================================================== */
.global _irq_handler
.type _irq_handler, %function
.align 4

_irq_handler:
    /* Save return address and SPSR */
    sub     lr, lr, #4          /* Adjust LR (ARM IRQ convention) */
    srsdb   sp!, #0x12          /* Save LR_irq and SPSR_irq to IRQ stack */

    /* Save caller-saved registers */
    push    {{r0-r3, r12}}

    /* Call Rust GIC dispatcher */
    bl      gic_irq_dispatch

    /* Restore registers */
    pop     {{r0-r3, r12}}

    /* Restore SPSR and return from IRQ */
    rfeia   sp!

.size _irq_handler, . - _irq_handler
