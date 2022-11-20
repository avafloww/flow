ENTRY(_start)

__kernel_addr_space_start_virt = 0xFFFFFFFF80000000;
__kernel_addr_space_end_virt = 0xFFFFFFFFFFFFFFFF;

PHDRS
{
    segment_code            PT_LOAD FLAGS(5); /* RX */
    segment_data            PT_LOAD FLAGS(6); /* RW */
}

SECTIONS
{
    . = __kernel_addr_space_start_virt;

    __code_start = .;
    .text :
    {
        KEEP(*(.text._start))
        *(.text._start_arguments)
        *(.text._start_rust)
        *(.text*)
    } :segment_code

    . += CONSTANT(MAXPAGESIZE);

    .rodata :
    {
        *(.rodata*)
    } :segment_code

    . += CONSTANT(MAXPAGESIZE);
    __code_end_exclusive = .;

    .data :
    {
        *(.data*)
    } :segment_data

    .bss :
    {
        __bss_start = .;
        *(.bss*);
        __bss_end_exclusive = .;
    } :segment_data

    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.eh_frame)
        *(.note .note.*)
        *(.comment*)
    }
}
