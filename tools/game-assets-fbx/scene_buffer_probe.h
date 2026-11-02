// Byte-range preflight for the audit's canonical, embedded, dense GLB route.
static uint64_t sourceUnsigned(const rapidjson::Value& object,const char* key,uint64_t fallback) {
    if(!object.HasMember(key))return fallback;
    require(object[key].IsUint64(),"invalid source buffer integer");return object[key].GetUint64();
}
static void verifySourceBuffers(const rapidjson::Document& doc,const char* path) {
    if(!doc.HasMember("buffers") && !doc.HasMember("bufferViews") && !doc.HasMember("accessors"))return;
    std::ifstream file(std::filesystem::u8path(path),std::ios::binary|std::ios::ate);require(bool(file),"cannot inspect source chunks");
    uint64_t size=uint64_t(file.tellg()),at=12,bin=0;bool hasBin=false;unsigned chunk=0;
    while(at<size) {
        require(size-at>=8,"truncated source chunk header");
        unsigned char header[8]{};file.seekg(std::streamoff(at));file.read(reinterpret_cast<char*>(header),8);
        auto word=[&](unsigned i){return uint32_t(header[i])|(uint32_t(header[i+1])<<8)|(uint32_t(header[i+2])<<16)|(uint32_t(header[i+3])<<24);};
        uint64_t length=word(0);uint32_t type=word(4);
        require(bool(file) && length%4==0 && length<=size-at-8,"source chunk exceeds file");
        if(type==0x004e4942){require(!hasBin && chunk==1,"invalid source BIN ordering");hasBin=true;bin=length;}
        if(type==0x4e4f534a)require(chunk==0,"duplicate source JSON chunk");
        at+=8+length;++chunk;
    }
    require(doc.HasMember("buffers") && doc["buffers"].IsArray() && doc["buffers"].Size()==1,"audit requires one embedded source buffer");
    const auto& buffer=doc["buffers"][0];require(buffer.IsObject() && !buffer.HasMember("uri"),"external source buffer unsupported by audit");
    uint64_t bytes=sourceUnsigned(buffer,"byteLength",0);
    require(hasBin && bytes>0 && bytes<=bin && bin-bytes<=3,"source buffer length differs from BIN");
    struct View {uint64_t offset,length,stride;};std::vector<View> views;
    if(doc.HasMember("bufferViews")) {
        require(doc["bufferViews"].IsArray(),"source buffer views must be an array");
        for(const auto& view:doc["bufferViews"].GetArray()) {
            require(view.IsObject(),"source buffer view must be an object");
            require(sourceUnsigned(view,"buffer",UINT64_MAX)==0,"source view buffer reference invalid");
            View v{sourceUnsigned(view,"byteOffset",0),sourceUnsigned(view,"byteLength",0),sourceUnsigned(view,"byteStride",0)};
            require(v.length>0 && v.offset<=bytes && v.length<=bytes-v.offset,"source view exceeds buffer");
            require(!view.HasMember("byteStride") || (v.stride>=4 && v.stride<=252 && v.stride%4==0),"source view stride invalid");
            views.push_back(v);
        }
    }
    uint64_t decoded=0;
    if(doc.HasMember("accessors")) {
        require(doc["accessors"].IsArray(),"source accessors must be an array");
        for(const auto& accessor:doc["accessors"].GetArray()) {
            require(accessor.IsObject(),"source accessor must be an object");
            require(!accessor.HasMember("sparse"),"sparse source accessor unsupported by audit");
            auto viewIndex=sourceUnsigned(accessor,"bufferView",UINT64_MAX);
            require(viewIndex<views.size(),"source accessor view reference invalid");const auto& view=views[size_t(viewIndex)];
            uint64_t count=sourceUnsigned(accessor,"count",0),offset=sourceUnsigned(accessor,"byteOffset",0);
            require(count>0 && count<=1000000,"source accessor count exceeds audit limit");
            auto component=sourceUnsigned(accessor,"componentType",0);uint64_t width=0;
            if(component==5120 || component==5121)width=1;
            if(component==5122 || component==5123)width=2;
            if(component==5125 || component==5126)width=4;
            require(width!=0,"source accessor component type invalid");
            require(accessor.HasMember("type") && accessor["type"].IsString(),"source accessor shape invalid");
            std::string type(accessor["type"].GetString(),accessor["type"].GetStringLength());uint64_t rows=0,cols=1;
            if(type=="SCALAR")rows=1;if(type=="VEC2")rows=2;if(type=="VEC3")rows=3;if(type=="VEC4")rows=4;
            if(type=="MAT2")rows=cols=2;if(type=="MAT3")rows=cols=3;if(type=="MAT4")rows=cols=4;
            require(rows!=0,"source accessor shape invalid");
            uint64_t column=rows*width;if(cols>1)column=(column+3)&~uint64_t(3);
            const uint64_t element=column*cols,last=column*(cols-1)+rows*width,stride=view.stride?view.stride:element;
            require(stride>=element && stride%width==0,"source accessor stride too small");
            require(offset<=view.length,"source accessor exceeds view");
            require(offset%width==0 && (view.offset+offset)%width==0 &&
                (cols==1 || (view.offset+offset)%4==0),"source accessor alignment invalid");
            require(offset<=view.length && last<=view.length-offset &&
                count-1<=(view.length-offset-last)/stride,"source accessor exceeds view");
            uint64_t allocation=count*rows*cols*8;
            require(allocation<=128*1024*1024-decoded,"source accessors exceed audit decoded budget");decoded+=allocation;
        }
    }
}
