ENTRY(_start)

__kernel_binary_start = 0xFFFFFFFFFC000000;

PHDRS
{
    segment_code            PT_LOAD FLAGS(5); /* RX */
    segment_data            PT_LOAD FLAGS(6); /* RW */
}

SECTIONS
{
    . = __kernel_binary_start;

    __kernel_code_end = .;
    .text : ALIGN(0x1000)
    {
        KEEP(*(.text._start))
        *(.text._start_arguments)
        *(.text._start_rust)
        *(.text*)
    } :segment_code

    .rodata : ALIGN (0x1000)
    {
        *(.rodata*)
    } :segment_code

    __kernel_code_end = .;

    . = ALIGN(0x1000);

    __kernel_data_start = .;

    .data : ALIGN (0x1000)
    {
        *(.data*)
    } :segment_data

    .bss : ALIGN (0x1000)
    {
        *(.bss*);
    } :segment_data

    __kernel_data_end = .;
    __kernel_binary_end = .;

    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.eh_frame)
        *(.note .note.*)
        *(.comment*)
    }
}
