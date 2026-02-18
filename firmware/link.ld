/* Linker script for Zynq 7020 bare metal firmware */
/* Memory map based on Xilinx UG585 */

MEMORY
{
    /* DDR3 SDRAM - 512MB (adjust for your board) */
    DDR (rwx) : ORIGIN = 0x00100000, LENGTH = 511M

    /* On-Chip Memory (OCM) - 256KB, used for DMA buffers */
    OCM (rwx) : ORIGIN = 0xFFFC0000, LENGTH = 256K

    /* Boot ROM (read-only, not used directly) */
    BOOTROM (rx) : ORIGIN = 0x00000000, LENGTH = 128K
}

/* Entry point */
ENTRY(_start)

/* Stack and heap sizes */
_stack_size = 64K;
_heap_size = 64K;

SECTIONS
{
    /* Code section - starts at 1MB to leave space for bootloader */
    .text : ALIGN(4)
    {
        _text_start = .;
        KEEP(*(.text.boot))    /* Boot code first */
        *(.text .text.*)
        *(.rodata .rodata.*)
        _text_end = .;
    } > DDR

    /* ARM exception vectors (optional - FSBL handles these) */
    .vectors : ALIGN(4)
    {
        _vectors_start = .;
        KEEP(*(.vectors))
        _vectors_end = .;
    } > DDR AT > DDR

    /* Initialized data */
    .data : ALIGN(4)
    {
        _data_start = .;
        *(.data .data.*)
        _data_end = .;
    } > DDR

    /* Uninitialized data (BSS) */
    .bss (NOLOAD) : ALIGN(4)
    {
        _bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        _bss_end = .;
    } > DDR

    /* Heap */
    .heap (NOLOAD) : ALIGN(8)
    {
        _heap_start = .;
        . += _heap_size;
        _heap_end = .;
    } > DDR

    /* Stack (grows down) */
    .stack (NOLOAD) : ALIGN(8)
    {
        _stack_end = .;
        . += _stack_size;
        _stack_start = .;
    } > DDR

    /* DMA buffers in OCM for cache coherency */
    .dma_buffers (NOLOAD) : ALIGN(32)
    {
        _dma_start = .;
        *(.dma_buffers)
        _dma_end = .;
    } > OCM

    /* Discard unwanted sections */
    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.ARM.extab*)
        *(.comment)
        *(.note.*)
    }
}

/* Provide symbols for startup code */
PROVIDE(_stack_top = _stack_start);
PROVIDE(_heap_bottom = _heap_start);
PROVIDE(_heap_top = _heap_end);
