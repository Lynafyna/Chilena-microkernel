/* Linker script untuk program CHN
 * Pastikan _start ada di offset 0x0 dari awal binary
 * sehingga entry_offset = 0 selalu benar
 */

ENTRY(_start)

SECTIONS {
    . = 0x0;

    /* .text harus pertama, dan _start harus di awal */
    .text : {
        *(.text._start)
        *(.text.hot)
        *(.text*)
    }

    /* .rodata langsung setelah .text */
    .rodata : {
        *(.rodata*)
    }

    /* .data setelah .rodata */
    .data : {
        *(.data*)
    }

    /* .bss terakhir */
    .bss : {
        *(.bss*)
        *(COMMON)
    }

    /* Buang semua section debug dan metadata */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.debug*)
        *(.comment*)
        *(.note*)
    }
}
