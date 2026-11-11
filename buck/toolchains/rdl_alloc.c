// The names are v0 mangled, with the disambiguator of the pinned rustc's std
// (rust_version in buck/tools/BUCK): `nm` of any binary's allocator shim shows it.
// std's default allocator entry points, which the allocator shim a Rust binary
// carries calls with hidden visibility, and which the standard library's shared
// library therefore cannot provide. The same as std::sys::alloc::unix.
#include <stdlib.h>
#include <string.h>

#define MIN_ALIGN 16
#define HIDDEN __attribute__((visibility("hidden")))

HIDDEN void *rdl_alloc(size_t size, size_t align) __asm__("_RNvCs6rREvFdRhLb_7___rustc11___rdl_alloc");
HIDDEN void rdl_dealloc(void *ptr, size_t size, size_t align) __asm__("_RNvCs6rREvFdRhLb_7___rustc13___rdl_dealloc");
HIDDEN void *rdl_realloc(void *ptr, size_t old, size_t align, size_t size) __asm__("_RNvCs6rREvFdRhLb_7___rustc13___rdl_realloc");
HIDDEN void *rdl_alloc_zeroed(size_t size, size_t align) __asm__("_RNvCs6rREvFdRhLb_7___rustc18___rdl_alloc_zeroed");

static void *aligned(size_t size, size_t align) {
  void *out = NULL;
  if (posix_memalign(&out, align < sizeof(void *) ? sizeof(void *) : align, size) != 0) return NULL;
  return out;
}

void *rdl_alloc(size_t size, size_t align) {
  if (align <= MIN_ALIGN && align <= size) return malloc(size);
  return aligned(size, align);
}

void rdl_dealloc(void *ptr, size_t size, size_t align) {
  (void)size;
  (void)align;
  free(ptr);
}

void *rdl_realloc(void *ptr, size_t old, size_t align, size_t size) {
  if (align <= MIN_ALIGN && align <= size) return realloc(ptr, size);
  void *out = aligned(size, align);
  if (out != NULL) {
    memcpy(out, ptr, old < size ? old : size);
    free(ptr);
  }
  return out;
}

void *rdl_alloc_zeroed(size_t size, size_t align) {
  if (align <= MIN_ALIGN && align <= size) return calloc(size, 1);
  void *out = aligned(size, align);
  if (out != NULL) memset(out, 0, size);
  return out;
}
