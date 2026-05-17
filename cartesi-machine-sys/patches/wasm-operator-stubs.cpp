// WASM operator new/delete stubs.
// wasi-sdk 33's clang generates these at link time, but Rust's linker
// (rust-lld) doesn't. Provide minimal implementations backed by malloc/free.

#include <cstddef>
#include <cstdlib>

void* operator new(std::size_t size) {
    if (void* p = std::malloc(size)) return p;
    std::abort();
}

void operator delete(void* ptr) noexcept { std::free(ptr); }
void operator delete(void* ptr, std::size_t) noexcept { std::free(ptr); }
