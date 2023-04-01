ENTRY(flow_init)

PHDRS
{
    segment_code            PT_LOAD FLAGS(5); /* RX */
    segment_data            PT_LOAD FLAGS(6); /* RW */
}

SECTIONS
{
    . = 0x0;

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

    . = ALIGN(0x1000);

    .data : ALIGN (0x1000)
    {
        *(.data*)
    } :segment_data

    .bss : ALIGN (0x1000)
    {
        *(.bss*);
    } :segment_data

    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.eh_frame)
        *(.note .note.*)
        *(.comment*)
    }
}
