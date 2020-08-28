OUTPUT_FORMAT(elf32-i386)
OUTPUT_ARCH(i386)
ENTRY(start)

SECTIONS {
    . = 0;
    .text : {
        . = ALIGN(4);
        *(.text)
        *(.text.*)
    }
    .rodata : {
        . = ALIGN(4);
        *(.rodata)
        *(.rodata.*)
    }
}
