MEMORY {
    /*RAM (rx) : ORIGIN = 0x20000000, LENGTH = 0x4000000*/
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 0x4000000
}

ENTRY(_start);

SECTIONS
{
  /* The program code and other data goes into RAM */
  .text :
  {
    . = ALIGN(4);
    KEEP (*(.text))
    *(.text)           /* .text sections (code) */
    *(.text*)          /* .text* sections (code) */

    KEEP (*(.init))

    . = ALIGN(4);
    _etext = .;        /* define a global symbols at end of code */
  } >RAM

  /* Constant data goes into RAM */
  .rodata :
  {
    . = ALIGN(4);
    *(.rodata)         /* .rodata sections (constants, strings, etc.) */
    *(.rodata*)        /* .rodata* sections (constants, strings, etc.) */
    . = ALIGN(4);
  } >RAM

  /* Initialized data sections goes into RAM, load LMA copy after code */
  .data : 
  {
    . = ALIGN(4);
    _sdata = .;        /* create a global symbol at data start */
    *(.data)           /* .data sections */
    *(.data*)          /* .data* sections */

    . = ALIGN(4);
    _edata = .;        /* define a global symbol at data end */
  } >RAM AT> RAM

  
  /* Uninitialized data section */
  . = ALIGN(4);
  .bss :
  {
    /* This is used by the startup in order to initialize the .bss secion */
    _sbss = .;         /* define a global symbol at bss start */
    PROVIDE(__bss_start__ = _sbss);
    *(.bss)
    *(.bss*)
    *(COMMON)

    . = ALIGN(4);
    _ebss = .;         /* define a global symbol at bss end */
    PROVIDE(__bss_end__ = _ebss);
  } >RAM

  PROVIDE(__stack = __bss_end__ + 0x80000);
}