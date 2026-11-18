#include <stdint.h>
uint8_t* __rust_probestack(uint64_t size) {
    uint8_t* sp;
    __asm__ volatile ("mov %%rsp, %0" : "=r" (sp));
    uint8_t* target = sp - size;
    while (sp > target) { sp -= 4096; *(volatile uint8_t*)sp; }
    return target;
}
