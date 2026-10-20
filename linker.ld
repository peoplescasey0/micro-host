/* POST CARD SPATIAL OS - LINKER SCRIPT FOR MULTIBOOT2 */
/* The .text.boot section (multiboot2 header + 32-bit entry) goes FIRST */

ENTRY(entry32)

SECTIONS
{
    /* Kernel loaded at 1MB - standard for multiboot */
    . = 1M;

    /* Boot section FIRST - contains multiboot2 header and 32-bit entry stub */
    /* This must be at the very start so GRUB finds the header */
    .text.boot : ALIGN(4K) {
        *(.text.boot)
    }

    /* Main code section */
    .text : ALIGN(4K) {
        *(.text)
        *(.text.*)
    }

    /* Read-only data */
    .rodata : ALIGN(4K) {
        *(.rodata)
        *(.rodata.*)
    }

    /* Initialized data */
    .data : ALIGN(4K) {
        *(.data)
        *(.data.*)
    }

    /* Uninitialized data */
    .bss : ALIGN(4K) {
        *(COMMON)
        *(.bss)
        *(.bss.*)
    }

    /* Discard unnecessary sections to keep binary clean */
    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.comment)
        *(.got)
        *(.got.plt)
    }

    kernel_end = .;
}
