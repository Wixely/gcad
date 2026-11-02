#include <map>
#include <set>
#include <array>
#include <tuple>
#include <queue>
#include <rapidjson/document.h>
#include <fstream>
// Independent face-fan traversal on geometry-only full-precision reference data.
// No normal values or smoothing groups are taken from the reference.
static void verifyCreaseNormals(const char *path, const char *referencePath) {
    Assimp::Importer importer; const auto *scene=importer.ReadFile(path,0);
    require(scene && scene->mNumMeshes==2,"expected hard/smooth plate fixture");
    auto *hardNode=scene->mRootNode->FindNode("hard"), *smoothNode=scene->mRootNode->FindNode("smooth");
    require(hardNode && smoothNode && hardNode->mNumMeshes==1 && smoothNode->mNumMeshes==1,"crease nodes missing");
    auto *hard=scene->mMeshes[hardNode->mMeshes[0]], *smooth=scene->mMeshes[smoothNode->mMeshes[0]];
    require(hard->mNumFaces==smooth->mNumFaces && hard->mNumFaces>100,"crease face count changed");
    using V=std::array<double,3>;
    std::ifstream input(referencePath); require(bool(input),"missing geometry reference");
    std::string json((std::istreambuf_iterator<char>(input)),std::istreambuf_iterator<char>());
    rapidjson::Document reference;reference.Parse<rapidjson::kParseFullPrecisionFlag>(json.c_str());
    require(!reference.HasParseError() && reference.IsObject() && reference.HasMember("version") && reference["version"]==1 &&
        reference.HasMember("positions") && reference["positions"].IsArray() && reference.HasMember("triangles") && reference["triangles"].IsArray(),"invalid geometry reference");
    require(reference["triangles"].Size()==hard->mNumFaces,"reference face count changed");
    auto referencePosition=[&](unsigned face,unsigned corner) {
        const auto &triangle=reference["triangles"][face];require(triangle.IsArray() && triangle.Size()==3 && triangle[corner].IsUint(),"invalid reference triangle");
        unsigned i=triangle[corner].GetUint();require(i<reference["positions"].Size(),"reference index out of bounds");
        const auto &p=reference["positions"][i];require(p.IsArray() && p.Size()==3,"invalid reference position");
        for(unsigned a=0;a<3;++a)require(p[a].IsNumber() && std::isfinite(p[a].GetDouble()),"nonfinite reference position");
        return V{p[0].GetDouble(),p[2].GetDouble(),-p[1].GetDouble()};
    };
    auto subtract=[](V a,V b){return V{a[0]-b[0],a[1]-b[1],a[2]-b[2]};};
    auto cross=[](V a,V b){return V{a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]};};
    auto dot=[](V a,V b){return a[0]*b[0]+a[1]*b[1]+a[2]*b[2];};
    auto normalize=[&](V a){double n=std::sqrt(dot(a,a));require(n>0 && std::isfinite(n),"degenerate imported face");for(auto &x:a)x/=n;return a;};
    std::map<std::tuple<double,double,double>,unsigned> points;
    std::vector<std::array<unsigned,3>> faces;
    std::vector<V> areas,normals;
    std::map<std::pair<unsigned,unsigned>,std::vector<unsigned>> edges;
    for(unsigned f=0;f<hard->mNumFaces;++f) {
        require(hard->mFaces[f].mNumIndices==3 && smooth->mFaces[f].mNumIndices==3,"nontriangle crease face");
        V p[3]; std::array<unsigned,3> ids;
        for(unsigned k=0;k<3;++k) {
            auto a=hard->mVertices[hard->mFaces[f].mIndices[k]],b=smooth->mVertices[smooth->mFaces[f].mIndices[k]];
            require(a==b,"normal rule changed triangle positions/order"); p[k]=referencePosition(f,k);
            require(a.x==float(p[k][0]/1000) && a.y==float(p[k][1]/1000) && a.z==float(p[k][2]/1000),"reference does not describe exported geometry");
            auto key=std::make_tuple(p[k][0],p[k][1],p[k][2]); auto result=points.emplace(key,unsigned(points.size()));ids[k]=result.first->second;
        }
        faces.push_back(ids); auto n=cross(subtract(p[1],p[0]),subtract(p[2],p[0]));areas.push_back(n);normals.push_back(normalize(n));
        for(unsigned k=0;k<3;++k) edges[std::minmax(ids[k],ids[(k+1)%3])].push_back(f);
    }
    size_t smoothed=0; double maxHard=0,maxSmooth=0;
    for(unsigned f=0;f<faces.size();++f) for(unsigned k=0;k<3;++k) {
        unsigned vertex=faces[f][k]; std::set<unsigned> visited{f}; std::queue<unsigned> pending;pending.push(f); V sum{};
        while(!pending.empty()) {
            unsigned current=pending.front();pending.pop();for(unsigned a=0;a<3;++a)sum[a]+=areas[current][a];
            auto tri=faces[current];
            for(unsigned e=0;e<3;++e) if(tri[e]==vertex || tri[(e+1)%3]==vertex) {
                auto &adjacent=edges[std::minmax(tri[e],tri[(e+1)%3])];
                if(adjacent.size()!=2)continue;
                unsigned next=adjacent[0]==current?adjacent[1]:adjacent[0];
                bool opposite=false;for(unsigned j=0;j<3;++j)opposite|=faces[next][j]==tri[(e+1)%3] && faces[next][(j+1)%3]==tri[e];
                if(opposite && dot(normals[current],normals[next])>=.5-1e-7 && visited.insert(next).second)pending.push(next);
            }
        }
        auto expected=normalize(sum);
        for(auto mesh:{hard,smooth}) {
            require(mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0),"crease shading attributes missing");
            auto i=mesh->mFaces[f].mIndices[k];auto n=mesh->mNormals[i],t=mesh->mTangents[i];
            V actual{n.x,n.y,n.z};require(std::abs(dot(actual,actual)-1)<1e-5,"nonunit exported normal");
            require(std::abs(n*t)<1e-5 && std::abs(t.SquareLength()-1)<1e-5,"invalid exported tangent");
            auto wanted=mesh==hard?normals[f]:expected;double delta=std::sqrt(dot(subtract(actual,wanted),subtract(actual,wanted)));
            require(std::isfinite(delta),"nonfinite normal comparison");
            if(mesh==hard)maxHard=std::max(maxHard,delta);else {maxSmooth=std::max(maxSmooth,delta);if(dot(actual,normals[f])<.99999)++smoothed;}
        }
    }
    require(smoothed>100,"smooth fixture lost visible normal variation");
    std::cout<<"Crease normals: "<<faces.size()<<" triangles each, "<<smoothed<<" smoothed corners; max normal-vector errors hard="<<maxHard<<", smooth="<<maxSmooth<<"\n";
    require(maxHard<1e-6 && maxSmooth<1e-6,"imported crease normals differ from independent face fans");
}
