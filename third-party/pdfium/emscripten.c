#include <math.h>
#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

uintptr_t __THREW__ = 0;
int __threwValue = 0;

static int temp_ret0 = 0;
static jmp_buf *catcher = NULL;

void setTempRet0(int value) { temp_ret0 = value; }

int getTempRet0(void) { return temp_ret0; }

void emscripten_longjmp(uintptr_t env, int value) {
    if (__THREW__ == 0) {
        __THREW__ = env;
        __threwValue = value;
    }
    if (catcher == NULL) {
        __builtin_trap();
    }
    longjmp(*catcher, 1);
}

static void caught(jmp_buf *outer) {
    catcher = outer;
    if (__THREW__ == 0) {
        __THREW__ = 1;
    }
}

int invoke_ii(int (*function)(int), int a) {
    jmp_buf here;
    jmp_buf *outer = catcher;
    catcher = &here;
    if (setjmp(here) == 0) {
        int result = function(a);
        catcher = outer;
        return result;
    }
    caught(outer);
    return 0;
}

int invoke_iii(int (*function)(int, int), int a, int b) {
    jmp_buf here;
    jmp_buf *outer = catcher;
    catcher = &here;
    if (setjmp(here) == 0) {
        int result = function(a, b);
        catcher = outer;
        return result;
    }
    caught(outer);
    return 0;
}

int invoke_iiii(int (*function)(int, int, int), int a, int b, int c) {
    jmp_buf here;
    jmp_buf *outer = catcher;
    catcher = &here;
    if (setjmp(here) == 0) {
        int result = function(a, b, c);
        catcher = outer;
        return result;
    }
    caught(outer);
    return 0;
}

int invoke_iiiii(int (*function)(int, int, int, int), int a, int b, int c, int d) {
    jmp_buf here;
    jmp_buf *outer = catcher;
    catcher = &here;
    if (setjmp(here) == 0) {
        int result = function(a, b, c, d);
        catcher = outer;
        return result;
    }
    caught(outer);
    return 0;
}

void invoke_viii(void (*function)(int, int, int), int a, int b, int c) {
    jmp_buf here;
    jmp_buf *outer = catcher;
    catcher = &here;
    if (setjmp(here) == 0) {
        function(a, b, c);
        catcher = outer;
        return;
    }
    caught(outer);
}

void invoke_viiii(void (*function)(int, int, int, int), int a, int b, int c, int d) {
    jmp_buf here;
    jmp_buf *outer = catcher;
    catcher = &here;
    if (setjmp(here) == 0) {
        function(a, b, c, d);
        catcher = outer;
        return;
    }
    caught(outer);
}

void emscripten_errn(const char *text, size_t length) { fwrite(text, 1, length, stderr); }

uintptr_t emscripten_stack_snapshot(void) { return 0; }

uint32_t emscripten_stack_unwind_buffer(uintptr_t pc, uintptr_t *buffer, uint32_t depth) {
    return 0;
}

const char *emscripten_pc_get_function(uintptr_t pc) { return NULL; }

int fiprintf(FILE *file, const char *format, ...) {
    va_list arguments;
    va_start(arguments, format);
    int written = vfprintf(file, format, arguments);
    va_end(arguments);
    return written;
}

int __fpclassifyf(float value) { return fpclassify(value); }
