// Versioned process boundary. Conversion is not advertised until integrated.
#include <assimp/version.h>
#include <iostream>
#include <string>
#include <exception>
#include <stdexcept>
#ifdef _WIN32
#include <windows.h>
#include <vector>
#endif

std::string convertStaged(const char* input,const char* output,bool leased);
std::string inspectMaterials(const char* input);
std::string inspectSkin(const char* input);

static int runHelper(int argc, char** argv) {
    if(argc==3 && std::string(argv[1])=="--inspect-skin-v1") {
        try { std::cout<<inspectSkin(argv[2])<<'\n';return 0; }
        catch(const std::exception& error) { std::cerr<<"Skin inspection failed: "<<error.what()<<'\n';return 1; }
    }
    if(argc==3 && std::string(argv[1])=="--inspect-materials-v1") {
        try { std::cout<<inspectMaterials(argv[2])<<'\n';return 0; }
        catch(const std::exception& error) {
            std::cerr<<"Material inspection failed: "<<error.what()<<'\n';return 1;
        }
    }
    if(argc==4 && (std::string(argv[1])=="--convert-v1" || std::string(argv[1])=="--convert-leased-v1")) {
        try { std::cout<<convertStaged(argv[2],argv[3],std::string(argv[1])=="--convert-leased-v1")<<'\n';return 0; }
        catch(const std::exception& error) {
            std::cerr<<"Conversion diagnostic: "<<error.what()<<'\n';
            std::cerr<<"{\"code\":\"conversion_failed\",\"message\":\"Staged conversion failed; package not published\"}\n";
            return 1;
        }
    }
    if (argc == 2 && std::string(argv[1]) == "--handshake") {
        std::cout << "{\"protocol\":1,\"helper\":\"gcad-fbx-helper\","
                  << "\"assimp_revision\":\"" GCAD_ASSIMP_REV "\","
                  << "\"assimp_diff_sha256\":\"" GCAD_ASSIMP_DIFF_SHA256 "\","
                  << "\"assimp_version\":[" << aiGetVersionMajor() << ','
                  << aiGetVersionMinor() << ',' << aiGetVersionPatch() << "],"
                  << "\"material_schema\":3,\"conversion_ready\":false,"
                  << "\"capabilities\":[\"staged-conversion-v1-experimental\",\"leased-staging-v1\",\"material-inspection-v1-experimental\",\"skin-inspection-v1-experimental\"]}\n";
        return 0;
    }
    std::cerr << "{\"code\":\"unsupported_request\",\"message\":\"Expected --handshake or --convert-v1 INPUT STAGE\"}\n";
    return 2;
}

#ifdef _WIN32
// Windows supplies UTF-16 arguments; the protocol and shared path helpers use
// UTF-8. Never route paths through the machine's ANSI code page.
int wmain(int argc, wchar_t** argv) {
    try {
        std::vector<std::string> arguments;
        for(int i=0;i<argc;++i) {
            int size=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,argv[i],-1,nullptr,0,nullptr,nullptr);
            if(size<=0)throw std::runtime_error("invalid Unicode argument");
            std::string value(size,'\0');
            if(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,argv[i],-1,value.data(),size,nullptr,nullptr)!=size)
                throw std::runtime_error("cannot encode Unicode argument");
            value.pop_back();arguments.push_back(std::move(value));
        }
        std::vector<char*> pointers;
        for(auto& argument:arguments)pointers.push_back(argument.data());
        return runHelper(argc,pointers.data());
    } catch(const std::exception& error) {
        std::cerr<<"Helper argument error: "<<error.what()<<'\n';return 1;
    }
}
#else
int main(int argc,char** argv) { return runHelper(argc,argv); }
#endif
