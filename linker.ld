/* POST CARD SPATIAL OS - LINKER SCRIPT FOR MULTIBOOT2 */
/* The .text.boot section (multiboot2 header + 32-bit entry) goes FIRST */

ENTRY(entry32)

SECTIONS
{
    /* Kernel loaded at 1MB - standard for multiboot */
    . = 1M;

    /* Boot section FIRST - contains multiboot2 header and 32-bit entry stub */
    /* This must be at the very start so GRUB finds the header */
    /* Section boundaries for paging (all 4K aligned) */
    .text.boot : ALIGN(4K) {
        __text_start = .;
        *(.text.boot)
    }

    /* Main code section */
    .text : ALIGN(4K) {
        *(.text)
        *(.text.*)
    }

    /* Read-only data */
    .rodata : ALIGN(4K) {
        __rodata_start = .;
        *(.rodata)
        *(.rodata.*)
    }

    /* Initialized data (and GOT: some calls, e.g. memcpy, go through GOT slots) */
    .data : ALIGN(4K) {
        __data_start = .;
        *(.data)
        *(.data.*)
        *(.got)
        *(.got.plt)
    }

    /* Uninitialized data */
    .bss : ALIGN(4K) {
        *(COMMON)
        *(.bss)
        *(.bss.*)
        . = ALIGN(4K);
    }

    /* Discard unnecessary sections to keep binary clean */
    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.comment)
    }

    kernel_end = .;
}
