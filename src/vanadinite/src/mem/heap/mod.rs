mod free_list;

use crate::utils::LinkerSymbol;

extern "C" {
    static HEAP_START: LinkerSymbol;
}
