OUTPUT_FORMAT(elf32-i386)
OUTPUT_ARCH(i386)
ENTRY(_start)

SECTIONS {
    . = 0x7000;
    .text.ap.entry : {
        /* Prevent the linker from removing this section. */
        KEEP(*(.text.ap.start))
    }

    /* Must be equal to KERNLINK */
    . = 0x80100000;

    .text : AT(0x100000) {
        *(.text .stub .text.* .gnu.linkonce.t.*)
    }

    PROVIDE(etext = .);	/* Define the 'etext' symbol to this value */

    .rodata : {
        *(.rodata .rodata.* .gnu.linkonce.r.*)
    }

    .stabstr : {
        PROVIDE(__STABSTR_BEGIN__ = .);
        *(.stabstr);
        PROVIDE(__STABSTR_END__ = .);
        BYTE(0) /* Force the linker to allocate space
                   for this section */
    }

    /* Adjust the address for the data segment to the next page */
    . = ALIGN(0x1000);

    /* Conventionally, Unix linkers provide pseudo-symbols
     * etext, edata, and end, at the end of the text, data, and bss.
     * For the kernel mapping, we need the address at the beginning
     * of the data section, but that's not one of the conventional
     * symbols, because the convention started before there was a
     * read-only rodata section between text and data. */
    PROVIDE(data = .);

    /* The data segment */
    .data : {
        *(.data .data.*)
    }

    PROVIDE(edata = .);

    .bss : {
        *(.bss .bss.*)
    }

    PROVIDE(kernel_end = .);

    /DISCARD/ : {
        *(.eh_frame .note.GNU-stack)
    }
}