ENTRY(_start)

/* 1968MB kernel heap (RW) */
__kernel_heap_start = 0xFFFFFFFF80000000;
__kernel_heap_end = 0xFFFFFFFFFAFFFFFF;

/* 16MB kernel stack (RW) */
__kernel_stack_start = 0xFFFFFFFFFB000000;
__kernel_stack_end = 0xFFFFFFFFFBFFFFFF;

/* 64MB kernel code (RX)/data+bss (RW) */
__kernel_binary_start = 0xFFFFFFFFFC000000;

PHDRS
{
    segment_code            PT_LOAD FLAGS(5); /* RX */
    segment_data            PT_LOAD FLAGS(6); /* RW */
}

SECTIONS
{
    . = __kernel_binary_start;

    __kernel_code_start = .;
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
