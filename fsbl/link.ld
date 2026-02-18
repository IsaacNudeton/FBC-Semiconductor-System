/* FSBL Linker Script - Runs from OCM before DDR is initialized */

MEMORY
{
    /* On-Chip Memory - 256KB total, but Boot ROM uses some */
    /* We get loaded at 0x0, but OCM is mapped there initially */
    OCM (rwx) : ORIGIN = 0x00000000, LENGTH = 192K

    /* Upper OCM for stack (Boot ROM doesn't touch this) */
    OCM_HIGH (rw) : ORIGIN = 0x00030000, LENGTH = 64K
}

ENTRY(_start)

_stack_size = 16K;

SECTIONS
{
    /* Vector table must be at 0x0 */
    .vectors : ALIGN(32)
    {
        KEEP(*(.vectors))
    } > OCM

    /* Boot code */
    .text : ALIGN(4)
    {
        *(.text.boot)
        *(.text .text.*)
        *(.rodata .rodata.*)
    } > OCM

    /* Initialized data */
    .data : ALIGN(4)
    {
        _data_start = .;
        *(.data .data.*)
        _data_end = .;
    } > OCM

    /* Uninitialized data */
    .bss (NOLOAD) : ALIGN(4)
    {
        _bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        _bss_end = .;
    } > OCM

    /* Stack in upper OCM */
    .stack (NOLOAD) : ALIGN(8)
    {
        _stack_end = .;
        . += _stack_size;
        _stack_top = .;
    } > OCM_HIGH

    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.ARM.extab*)
        *(.comment)
        *(.note.*)
    }
}

PROVIDE(_stack_top = ORIGIN(OCM_HIGH) + LENGTH(OCM_HIGH));
