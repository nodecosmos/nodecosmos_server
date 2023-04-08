1. Stack memory: This is a type of memory that is used for storing function call frames and local variables during program execution. It is a LIFO (Last-In, First-Out) data structure, where the most recent item added to the stack is the first to be removed.
    - Activation records: These are the data structures that represent function call frames on the stack. They typically include information about the function's parameters, local variables, return address, and other metadata.

2. Heap memory: This is a type of memory that is used for dynamically allocating memory during program execution. It is a region of memory that is managed manually by the programmer, and is used for storing data structures like arrays, structs, and other dynamically allocated data.
    - Contiguous memory: This is a type of memory layout where the data is stored in a contiguous block of memory. It is often used for arrays and other data structures where elements are stored in a fixed-size chunk of memory.
    - Non-contiguous memory: This is a type of memory layout where the data is stored in non-contiguous blocks of memory. It is often used for data structures like linked lists and trees, where the elements are not necessarily stored in a fixed-size chunk of memory.
    - Garbage-collected memory: This is a type of memory management where the system automatically deallocates memory that is no longer being used by the program. It is typically used in higher-level programming languages like Java and Python.

3. Static memory: This is a type of memory that is allocated at compile time and is used for storing global variables, constants, and other static data.
    - Read-only memory: This is a type of memory that is used for storing data that is read-only and cannot be modified at runtime. It is typically used for storing constants and other immutable data.

4. Code memory: This is a type of memory that is used for storing executable code, such as the instructions for functions and other program logic. It is typically read-only and is managed automatically by the compiler or linker.

5. Memory-mapped I/O: This is a technique for accessing hardware devices and other external resources as if they were memory locations. It is typically used for devices that require fast and efficient access, such as graphics cards and network interfaces.

6. Virtual memory: This is a technique for expanding the available memory of a system by using secondary storage like hard drives as a backup for the main memory. It is typically managed by the operating system and is used for swapping out less frequently used memory to disk and loading it back into memory as needed.



Programs determine the usage of all types of memory, but the difference is that the usage of stack, static, and code memory sections is determined at compile-time, while the usage of the heap memory section is determined at runtime.


```rust
struct SomeStruct {
    data: Vec<i32>,
}

impl SomeStruct {
    fn new() -> SomeStruct {
        SomeStruct {
            data: vec![1, 2, 3], // stored on Contiguous memory
        }
    }

    fn modify_data(&mut self) {
        self.data.push(4); // modify the internal state of SomeStruct
                           // This can cause changing pointer of data which will change pointer of SomeStruct
                           // eventually causing pointer of SomeStruct to be invalid if it is stored in static memory
    }
}

fn return_static_struct() -> &'static SomeStruct {
    let mut s = SomeStruct::new();
    let p = Box::into_raw(Box::new(s));
    unsafe {
        (*p).modify_data(); // modify SomeStruct after creating the raw pointer
        &*p
    }
}

fn main() {
    let s = return_static_struct();
    println!("{:?}", s.data); // potential use-after-free or null pointer dereference error
}
```
