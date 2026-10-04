#pragma pattern_limit 2000000
/* By Watertoon */
/* Target versions: 0x407,0x408 */

u32 g_version = 0;
u32 g_string_pool_offset = 0;

struct Vector3f {
    float x;
    float y;
    float z;
};

struct Guid {
    u32 guid0;
    u16 guid1;
    u16 guid2;
    u8 guid3[8];
};

struct AinbString {
    u32 offset;

    char o_string[] @ g_string_pool_offset + offset;
};
struct AinbPtr<T> {
    u32 offset;

    if (0 < offset) {
        T o_value @ offset [[inline]];
    }
};
struct AinbArray<T> {
    u32 count;
    T array[count];
};

/* Command */
struct ResAinbCommand {
    AinbString name;
    Guid guid;
    u16 element_id;
    u16 other_element_id;
};

/* Property */
u32 g_property_table = 0;
u32 g_input_output_table = 0;
u32 g_transition_table = 0;
u32 g_multi_param_table = 0;
enum PropertyType : u32 {
    S32 = 0,
    Bool = 1,
    Float = 2,
    String = 3,
    Vector3f = 4,
    Pointer = 5,
};

struct ResAinbPropertyTable {
    u32 type_offset_array[6];
};

bitfield AinbPropertyFlags {
    index : 16;
    reserve0 : 7;
    is_pulse_tls : 1;
    is_ptr_flag0 : 1;
    reserve1 : 7;
};

struct Ptr {};
struct ResAinbProperty<T> {
    AinbString name;

    if (typenameof(T) == "Ptr") {
        AinbString class_name;
        AinbPropertyFlags flags;
    } else {
        AinbPropertyFlags flags;
        if (typenameof(T) == "bool") {
            u32 value;
        } else {
            T value;
        }
    }
};

/* Multi Param */
struct ResAinbMultiParam {
    u16 input_element_id;
    u16 input_element_output_plug_id;
    AinbPropertyFlags flags;
};

/* Input output */
struct ResAinbIOOffset {
    u32 offset_input;
    u32 offset_output;
};
struct ResAinbInputOutputTable {
    ResAinbIOOffset offset_array[6];
};
struct ResAinbInput<T> {
    AinbString name;
    if (typenameof(T) == "Ptr") {
        AinbString class_name;
    }
    s16 input_element_or_base_multi_param_id;
    s16 input_element_output_plug_or_multi_param_count_id;
    AinbPropertyFlags flags;

    if (typenameof(T) == "Ptr") {
        u32 reserve0;
    } else if (typenameof(T) == "bool") {
        u32 value;
    } else {
        T value;
    }

    if (input_element_or_base_multi_param_id <= -100) {
        ResAinbMultiParam o_multi_param_array[input_element_output_plug_or_multi_param_count_id] @ g_multi_param_table + 8 * (-100 - input_element_or_base_multi_param_id);
    }
};

bitfield AinbOutputFlags {
    name : 31;
    is_set_ptr_flag0 : 1;
};
struct ResAinbOutput<T> {
    AinbOutputFlags flags;
    if (typenameof(T) == "Ptr") {
        AinbString class_name;
    }

    char o_name[] @ flags.name + g_string_pool_offset;
};

/* Plugs */
enum PlugType : u32 {
    Generic = 0,
    Reserve1 = 1,
    Child = 2,
    Transition = 3,
    String = 4,
    Int = 5, /* Input plug? */
    Reserve4 = 6,
    Reserve5 = 7,
    Reserve6 = 8,
    Reserve7 = 9,
};

bitfield AinbTransitionFlags {
    is_valid : 8;
    reserve0 : 23;
    is_update_post_calc : 1; /* Changes the node post current command calc instead of before next command calc */
};

struct ResAinbTransition {
    AinbTransitionFlags flags;

    if (flags.is_valid == 0) {
        AinbString string;
    }
};

enum ElementType : u16 {
    ApplicationDefined = 0,
    S32Selector,
    Sequential,
    Simultaneous,
    F32Selector,
    StringSelector,
    RandomSelector,
    BoolSelector,
    Fork,
    Join,
    Alert,
    Expression = 20,
    ModuleIF_Input_S32 = 100,
    ModuleIF_Input_F32 = 101,
    ModuleIF_Input_Vec3f = 102,
    ModuleIF_Input_String = 103,
    ModuleIF_Input_Bool = 104,
    ModuleIF_Input_Ptr = 105,
    ModuleIF_Output_S32 = 200,
    ModuleIF_Output_F32 = 201,
    ModuleIF_Output_Vec3f = 202,
    ModuleIF_Output_String = 203,
    ModuleIF_Output_Bool = 204,
    ModuleIF_Output_Ptr = 205,
    ModuleIF_Child = 300,
    StateEnd = 400,
    SplitTiming = 500,
};
ElementType s_last_element_type = 0;

struct AinbPlugProperty<T> {
    u32 flags;
    match(typenameof(T)) {
        (typenameof(s32)): {
            s32 value;
        }
        (typenameof(AinbString)): {
            AinbString value;
        }
    }
};

struct ResAinbPlug<auto T> {
    s16 element_id;
    u16 reserve0;

    match (T) {
        (PlugType::Generic): {
            AinbString name;
        }
        (PlugType::Child): {
            AinbString name;
            if (s_last_element_type == ElementType::StringSelector) {
                AinbPlugProperty<AinbString> value;
            }

            u32 reserve1;
            u32 reserve2;
        }
        (PlugType::Transition): {
            u32 transition_id;

            AinbPtr<ResAinbTransition> o_transition @ g_transition_table + 4 * transition_id;
        }
        (PlugType::String): {
            AinbString name;

            if (s_last_element_type == ElementType::StringSelector) {
                AinbPlugProperty<AinbString> value;
            }
        }

        (PlugType::Int): {
            AinbString name;

            u32 reserve1;
            u32 reserve2;
        }
    }
};

/* Element param */

struct ResAinbElementPropertyEntry<auto T> {
    u32 base_index;
    u32 count;

    ResAinbPropertyTable o_table @ g_property_table [[hidden]];
    match (T) {
        (PropertyType::S32): {
            ResAinbProperty<s32> o_property_array[count] @ o_table.type_offset_array[T] + base_index * 0xc;
        }
        (PropertyType::Float): {
            ResAinbProperty<float> o_property_array[count] @ o_table.type_offset_array[T] + base_index * 0xc;
        }
        (PropertyType::Bool): {
            ResAinbProperty<bool> o_property_array[count] @ o_table.type_offset_array[T] + base_index * 0xc;
        }
        (PropertyType::String): {
            ResAinbProperty<AinbString> o_property_array[count] @ o_table.type_offset_array[T] + base_index * 0xc;
        }
        (PropertyType::Vector3f): {
            ResAinbProperty<Vector3f> o_property_array[count] @ o_table.type_offset_array[T] + base_index * 0x14;
        }
        (PropertyType::Pointer): {
            ResAinbProperty<Ptr> o_property_array[count] @ o_table.type_offset_array[T] + base_index * 0xc;
        }
    }
};
struct ResAinbElementProperties {
    ResAinbElementPropertyEntry<PropertyType::S32> entry_s32;
    ResAinbElementPropertyEntry<PropertyType::Bool> entry_bool;
    ResAinbElementPropertyEntry<PropertyType::Float> entry_float;
    ResAinbElementPropertyEntry<PropertyType::String> entry_string;
    ResAinbElementPropertyEntry<PropertyType::Vector3f> entry_vec3f;
    ResAinbElementPropertyEntry<PropertyType::Pointer> entry_ptr;
};
struct ResAinbElementInputEntry<auto T> {
    u32 base_index;
    u32 count;

    ResAinbInputOutputTable o_table @ g_input_output_table [[hidden]];
    match (T) {
        (PropertyType::S32): {
            ResAinbInput<s32> o_input_array[count] @ o_table.offset_array[T].offset_input + base_index * 0x10;
        }
        (PropertyType::Bool): {
            ResAinbInput<bool> o_input_array[count] @ o_table.offset_array[T].offset_input + base_index * 0x10;
        }
        (PropertyType::Float): {
            ResAinbInput<float> o_input_array[count] @ o_table.offset_array[T].offset_input + base_index * 0x10;
        }
        (PropertyType::String): {
            ResAinbInput<AinbString> o_input_array[count] @ o_table.offset_array[T].offset_input + base_index * 0x10;
        }
        (PropertyType::Vector3f): {
            ResAinbInput<Vector3f> o_input_array[count] @ o_table.offset_array[T].offset_input + base_index * 0x18;
        }
        (PropertyType::Pointer): {
            ResAinbInput<Ptr> o_input_array[count] @ o_table.offset_array[T].offset_input + base_index * 0x14;
        }
    }
};
struct ResAinbElementOutputEntry<auto T> {
    u32 base_index;
    u32 count;

    ResAinbInputOutputTable o_table @ g_input_output_table [[hidden]];
    match (T) {
        (PropertyType::S32): {
            ResAinbOutput<s32> o_output_array[count] @ o_table.offset_array[T].offset_output + base_index * 0x4;
        }
        (PropertyType::Bool): {
            ResAinbOutput<bool> o_output_array[count] @ o_table.offset_array[T].offset_output + base_index * 0x4;
        }
        (PropertyType::Float): {
            ResAinbOutput<float> o_output_array[count] @ o_table.offset_array[T].offset_output + base_index * 0x4;
        }
        (PropertyType::String): {
            ResAinbOutput<AinbString> o_output_array[count] @ o_table.offset_array[T].offset_output + base_index * 0x4;
        }
        (PropertyType::Vector3f): {
            ResAinbOutput<Vector3f> o_output_array[count] @ o_table.offset_array[T].offset_output + base_index * 0x4;
        }
        (PropertyType::Pointer): {
            ResAinbOutput<Ptr> o_output_array[count] @ o_table.offset_array[T].offset_output + base_index * 0x8;
        }
    }
};
struct ResAinbElementInputOutput {
    ResAinbElementInputEntry<PropertyType::S32> input_entry_s32;
    ResAinbElementOutputEntry<PropertyType::S32> output_entry_s32;
    ResAinbElementInputEntry<PropertyType::Bool> input_entry_bool;
    ResAinbElementOutputEntry<PropertyType::Bool> output_entry_bool;
    ResAinbElementInputEntry<PropertyType::Float> input_entry_float;
    ResAinbElementOutputEntry<PropertyType::Float> output_entry_float;
    ResAinbElementInputEntry<PropertyType::String> input_entry_string;
    ResAinbElementOutputEntry<PropertyType::String> output_entry_string;
    ResAinbElementInputEntry<PropertyType::Vector3f> input_entry_vec3f;
    ResAinbElementOutputEntry<PropertyType::Vector3f> output_entry_vec3f;
    ResAinbElementInputEntry<PropertyType::Pointer> input_entry_ptr;
    ResAinbElementOutputEntry<PropertyType::Pointer> output_entry_ptr;
};

u32 g_plug_array = 0;
struct ResAinbElementPlugEntry<auto T> {
    u8 count;
    u8 base_index;

    AinbPtr<ResAinbPlug<T>> o_plug_array[count] @ g_plug_array + base_index * 4;
};
struct ResAinbElementPlugs {
    ResAinbElementPlugEntry<PlugType::Generic> entry_generic;
    ResAinbElementPlugEntry<PlugType::Reserve1> entry_reserve1;
    ResAinbElementPlugEntry<PlugType::Child> entry_child;
    ResAinbElementPlugEntry<PlugType::Transition> entry_transition;
    ResAinbElementPlugEntry<PlugType::String> entry_string;
    ResAinbElementPlugEntry<PlugType::Int> entry_int;
    ResAinbElementPlugEntry<PlugType::Reserve4> entry_reserve4;
    ResAinbElementPlugEntry<PlugType::Reserve5> entry_reserve5;
    ResAinbElementPlugEntry<PlugType::Reserve6> entry_reserve6;
    ResAinbElementPlugEntry<PlugType::Reserve7> entry_reserve7;
};

struct ResAinbElementParam<auto T> {
    g_plug_array = addressof(this) + 0xa4;

    ResAinbElementProperties properties;
    ResAinbElementInputOutput input_output;
    ResAinbElementPlugs plugs;

};


/* Element */
u32 g_query_id_table = 0;
u32 g_attachment_id_table = 0;
bitfield AinbElementFlags {
    is_query : 1;
    is_module_caller : 1;
    is_resident_initialized : 1; /* Node is initialized for ainb duration */
    is_query_multi_param : 1;
    reserve0 : 4;
};
struct ResAinbElement {
    ElementType element_type;
    s_last_element_type = element_type;
    u16 element_id;
    u16 attachment_count;
    AinbElementFlags flags;
    u8 reserve0;
    AinbString name;
    u32 name_hash_murmur3;
    u32 reserve1;
    AinbPtr<ResAinbElementParam<element_type>> element_param;
    u16 exb_field_count;
    u16 exb_value_size;
    u16 multi_param_count;
    u16 reserve2;
    u32 base_attachment_id;
    u16 base_query_id;
    u16 query_count;
    u16 reserve3;
    u16 reserve4;
    Guid guid;


    u32 o_attachment_id_array[attachment_count] @ g_attachment_id_table + 4 * base_attachment_id;
    u32 o_query_id_array[query_count] @ g_query_id_table + 4 * base_query_id;
};

/* Blackboard */
enum BlackboardType : u32 {
    String,
    S32,
    U32,
    Float,
    Bool,
    Vector3f,
    Pointer,
};

enum InheritMode : u8 {
    InheritFromRoot = 0,
    InheritFromParent = 1,
    DontInherit = 2,
};


bitfield AinbBlackboardFlags {
    name : 22;
    InheritMode inherit_mode : 2;
    file_ref_idx : 7;
    bool has_file_ref : 1;
};

struct FileReferenceEntry {
    AinbString file_path;
    u32 file_path_hash;
    u32 file_name_hash;
    u32 file_ext_hash;
};

struct ResAinbBlackboardEntry {
    AinbBlackboardFlags flags;
    AinbString reserve0;

    char o_name[] @ g_string_pool_offset + flags.name;

   if (flags.has_file_ref) {
        FileReferenceEntry o_file_ref
            @ g_bb_file_ref_offset + flags.file_ref_idx * 0x10;
    }
};

struct ResAinbBlackboardTypeEntry {
    u16 count;
    u16 base_index;
    u16 base_offset;
    u16 reserve1;
};
u32 g_bb_file_ref_offset = 0;
struct ResAinbBlackboard {
    ResAinbBlackboardTypeEntry entry_string;
    ResAinbBlackboardTypeEntry entry_int;
    if (0x408 <= g_version) {
        ResAinbBlackboardTypeEntry entry_u32;
    }
    ResAinbBlackboardTypeEntry entry_float;
    ResAinbBlackboardTypeEntry entry_bool;
    ResAinbBlackboardTypeEntry entry_vector3f;
    ResAinbBlackboardTypeEntry entry_void;

  g_bb_file_ref_offset = addressof(this)
        + 0x38
        + ((entry_void.count + entry_void.base_index) * 0x8)
        + entry_vector3f.base_offset
        + entry_vector3f.count * 0xC;

    ResAinbBlackboardEntry o_string_entry_array[entry_string.count] @ addressof(this) + 0x38 + entry_string.base_index * 0x8;
    ResAinbBlackboardEntry o_int_entry_array[entry_int.count] @ addressof(this) + 0x38 + entry_int.base_index * 0x8;
    if (0x408 <= g_version) {
        ResAinbBlackboardEntry o_u32_entry_array[entry_u32.count] @ addressof(this) + 0x38 + entry_u32.base_index * 0x8;
    }
    ResAinbBlackboardEntry o_float_entry_array[entry_float.count] @ addressof(this) + 0x38 + entry_float.base_index * 0x8;
    ResAinbBlackboardEntry o_bool_entry_array[entry_bool.count] @ addressof(this) + 0x38+ entry_bool.base_index * 0x8;
    ResAinbBlackboardEntry o_vec3f_entry_array[entry_vector3f.count] @ addressof(this) + 0x38 + entry_vector3f.base_index * 0x8;
    ResAinbBlackboardEntry o_ptr_entry_array[entry_void.count] @ addressof(this) + 0x38 + entry_void.base_index * 0x8;

    AinbString o_string_value_array[entry_string.count] @ addressof(this) + 0x38 + entry_string.base_offset + ((entry_void.count + entry_void.base_index) * 0x8);
    s32 o_int_value_array[entry_int.count] @ addressof(this) + 0x38 + entry_int.base_offset + ((entry_void.count + entry_void.base_index) * 0x8);
    if (0x408 <= g_version) {
        u32 o_u32_value_array[entry_u32.count] @ addressof(this) + 0x38 + entry_u32.base_offset + ((entry_void.count + entry_void.base_index) * 0x8);
    }
    float o_float_value_array[entry_float.count] @ addressof(this) + 0x38 + entry_float.base_offset + ((entry_void.count + entry_void.base_index) * 0x8);
    u32 o_bool_value_array[entry_bool.count] @ addressof(this) + 0x38 + entry_bool.base_offset + ((entry_void.count + entry_void.base_index) * 0x8);
    Vector3f o_vector3f_value_array[entry_vector3f.count] @ addressof(this) + 0x38 + entry_vector3f.base_offset + ((entry_void.count + entry_void.base_index) * 0x8);
};

/* Attachment */
struct ResAinbAttachmentParam {
    u8 is_update;
    u24 reserve0;
    ResAinbElementProperties properties;
};
struct ResAinbAttachment {
    AinbString name;
    AinbPtr<ResAinbAttachmentParam> attachment_param;
    u16 exb_field_count;
    u16 exb_value_size;
    u32 name_hash_murmur3;
};

/* Enum Relocation */
struct ResAinbEnumRelocation {
    AinbPtr<s32> value_to_replace;
    AinbString enum_class;
    AinbString enum_value;
};

/* Module Caller */
struct ResAinbModuleCaller {
    AinbString path;
    AinbString category;
    u32 ainb_count;
};

/* External Action */
struct ResAinbExternalAction {
    AinbString type;
    AinbString slot;
    AinbString action;
};

/* Module Caller link */
struct ResAinbModuleCallerLink {
    u32 id;
    u32 parent_id;
};

/* Replace table */
enum AinbFixupType : u8 {
    RemovePlugChild  = 0,
    ReplacePlugChild = 1,
    RemoveAttachment = 2,
};
struct ResAinbFixup {
    AinbFixupType type;
    u8  reserve0;
    u16 target_element_id;

    if (type == AinbFixupType::RemovePlugChild) {
        u16 base_plug_child_id;
        u16 reserve1;
    } else if (type == AinbFixupType::ReplacePlugChild) {
        u16 base_plug_child_id;
        u16 replace_element_id;
    } else if (type == AinbFixupType::RemoveAttachment) {
        u16 base_attachment_id;
        u16 reserve1;
    } else { /* Should not exist */
        u16 reserve1;
        u16 reserve2;
    }
};
struct ResAinbFixupTable {
    u8 runtime_init_guard;
    u8 reserve0;
    u16 fixup_count;
    u16 override_element_count;
    u16 override_attachment_count;
    ResAinbFixup fixup_array[fixup_count];
};

/* Header */
struct ResAinb {
    u32 o_string_pool_offset @ 0x24 [[hidden]];
    g_string_pool_offset = o_string_pool_offset;

    u32 magic;
    u32 version;
    g_version = version;
    AinbString name;
    u32 command_count;
    u32 element_count;
    u32 query_count;
    u32 attachment_count;
    u32 element_output_count;
    AinbPtr<ResAinbBlackboard> blackboard;
    u32 string_pool;
    AinbPtr<AinbArray<ResAinbEnumRelocation>> enum_relocation_array;
    AinbPtr<ResAinbPropertyTable> property_table;
    u32 transition_table;
    AinbPtr<ResAinbInputOutputTable> input_output_table;
    u32 multi_param_array;
    u32 attachment_array;
    u32 attachment_id_array;
    u32 exb;
    AinbPtr<ResAinbFixupTable> fixup_table;
    u32 query_id_array;
    u32 reserve0;
    u32 reserve1;
    u32 reserve2;
    AinbPtr<AinbArray<ResAinbModuleCaller>> module_caller_array;
    AinbString category;
    u32 reserve3;
    AinbPtr<AinbArray<ResAinbExternalAction>> external_action_array;
    AinbPtr<AinbArray<s32>> reserve4;
    AinbPtr<ResAinbModuleCallerLink> module_caller_link;

    g_property_table = property_table.offset;
    g_transition_table = transition_table;
    g_input_output_table = input_output_table.offset;
    g_multi_param_table = multi_param_array;
    g_query_id_table = query_id_array;
    g_attachment_id_table = attachment_id_array;

    ResAinbCommand o_command_array[command_count] @ 0x74;
    ResAinbElement o_element_array[element_count] @ addressof(o_command_array) + sizeof(o_command_array);
    ResAinbAttachment o_attachment_array[attachment_count] @ attachment_array;
};

ResAinb header @ 0x0;
