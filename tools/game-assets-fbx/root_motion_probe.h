// Independent glTF extras parsing and imported-animation recomposition.
#include <rapidjson/document.h>
#include <fstream>
#include <iterator>
#include <cstdint>

static aiMatrix4x4 rootMotionPose(const aiAnimation* clip,const aiNode* root,double seconds,aiVector3D motion={}) {
    const aiNodeAnim* track=nullptr;
    for(unsigned i=0;i<clip->mNumChannels;++i)if(clip->mChannels[i]->mNodeName==root->mName)track=clip->mChannels[i];
    require(track && clip->mNumChannels==1 && clip->mTicksPerSecond>0,"root-motion channel changed");
    double ticks=seconds*clip->mTicksPerSecond;
    auto vectorAt=[&](const aiVectorKey* keys,unsigned count) {
        require(count>0,"root-motion vector keys missing");
        unsigned a=0;while(a+1<count && keys[a+1].mTime<=ticks)++a;unsigned b=std::min(a+1,count-1);
        float f=a==b?0:float(std::clamp((ticks-keys[a].mTime)/(keys[b].mTime-keys[a].mTime),0.,1.));
        require(keys[a].mInterpolation==aiAnimInterpolation_Linear,"root-motion fixture interpolation changed");
        return keys[a].mValue*(1-f)+keys[b].mValue*f;
    };
    auto position=vectorAt(track->mPositionKeys,track->mNumPositionKeys)+motion;
    auto scale=vectorAt(track->mScalingKeys,track->mNumScalingKeys);
    require(track->mNumRotationKeys==2,"root-motion rotation keys changed");
    float f=float(std::clamp(ticks/clip->mDuration,0.,1.));aiQuaternion rotation;
    aiQuaternion::Interpolate(rotation,track->mRotationKeys[0].mValue,track->mRotationKeys[1].mValue,f);rotation.Normalize();
    return aiMatrix4x4(scale,rotation,position);
}
static void verifyRootMotion(const char* path) {
    std::ifstream input(path,std::ios::binary);std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(input)),{});
    require(bytes.size()>=28,"root-motion GLB missing");
    auto word=[&](size_t i){require(i+4<=bytes.size(),"root-motion GLB truncated");return uint32_t(bytes[i])|(uint32_t(bytes[i+1])<<8)|(uint32_t(bytes[i+2])<<16)|(uint32_t(bytes[i+3])<<24);};
    require(word(0)==0x46546c67 && word(4)==2 && word(8)==bytes.size() && word(16)==0x4e4f534a,"root-motion GLB header changed");
    auto length=word(12);require(length<=bytes.size()-20,"root-motion JSON truncated");
    rapidjson::Document doc;doc.Parse(reinterpret_cast<const char*>(bytes.data()+20),length);
    require(!doc.HasParseError() && doc.HasMember("extras") && doc["extras"].HasMember("gcad_root_motion"),"root-motion metadata missing");
    const auto& motion=doc["extras"]["gcad_root_motion"];
    require(motion["schema"].GetUint()==1 && motion["source_animation"].GetUint()==0 && motion["in_place_animation"].GetUint()==1 && motion["node"].GetUint()==0,"root-motion references changed");
    require(std::string(doc["nodes"][0]["name"].GetString())=="root","metadata refers to wrong root");
    require(std::string(motion["length_unit"].GetString())=="metre" && std::string(motion["up_axis"].GetString())=="Y" && std::string(motion["handedness"].GetString())=="right","root-motion coordinate convention changed");
    require(!motion["rotation_extracted"].GetBool() && std::string(motion["interpolation"].GetString())=="linear","root-motion extraction mode changed");
    require(motion["translation_axes"][0].GetBool() && !motion["translation_axes"][1].GetBool() && motion["translation_axes"][2].GetBool(),"root-motion axes not converted");
    const auto& times=motion["times_seconds"];const auto& values=motion["displacements"];
    require(times.Size()==3 && values.Size()==3 && motion["duration_seconds"].GetDouble()==2.,"root-motion timing changed");
    for(unsigned i=0;i<3;++i) {
        require(times[i].GetDouble()==i && values[i].Size()==3,"root-motion keys changed");
        require(std::abs(values[i][0].GetDouble()-.05*i)<1e-12 && values[i][1].GetDouble()==0 && std::abs(values[i][2].GetDouble()+.025*i)<1e-12,"root-motion displacement units/sign changed");
    }
    aiVector3D cycle(float(motion["loop_displacement"][0].GetDouble()),float(motion["loop_displacement"][1].GetDouble()),float(motion["loop_displacement"][2].GetDouble()));
    require((cycle-aiVector3D(.1f,0,-.05f)).Length()<1e-8,"root-motion cycle displacement changed");
    auto displacement=[&](double seconds) {
        double clamped=std::clamp(seconds,0.,2.);unsigned a=clamped<1?0:1;
        double f=clamped-times[a].GetDouble();aiVector3D p;
        for(unsigned k=0;k<3;++k)p[k]=float(values[a][k].GetDouble()*(1-f)+values[a+1][k].GetDouble()*f);
        return p;
    };
    Assimp::Importer importer;auto* scene=importer.ReadFile(path,0);
    require(scene && scene->mNumAnimations==2 && scene->mNumMeshes==1,"root-motion imported scene changed");
    auto* root=scene->mRootNode->FindNode("root");auto* plate=scene->mRootNode->FindNode("plate");auto* other=scene->mRootNode->FindNode("unrelated");
    require(root && plate && other && plate->mParent==root && other->mParent!=root && plate->mNumMeshes==1 && other->mNumMeshes==1 && plate->mMeshes[0]==other->mMeshes[0],"root-motion hierarchy/instance isolation changed");
    auto* source=scene->mAnimations[0];auto* inPlace=scene->mAnimations[1];
    require(std::string(source->mName.C_Str())=="travel" && std::string(inPlace->mName.C_Str())=="travel-in-place","root-motion clips changed");
    double maxError=0,maxSuppressedError=0,maxAnalyticError=0;
    auto* mesh=scene->mMeshes[plate->mMeshes[0]];
    require(mesh->mNumVertices==1240 && mesh->mNumFaces==1276,"root-motion CAD plate changed");
    for(unsigned sample=0;sample<=16;++sample) {
        double time=sample/8.;
        auto original=rootMotionPose(source,root,time)*plate->mTransformation;
        auto stationary=rootMotionPose(inPlace,root,time);
        require(std::abs(stationary.a4-.01)<1e-7 && std::abs(stationary.c4+.02)<1e-7 && std::abs(stationary.b4-(.03+.005*time))<1e-7,"in-place translation axes changed");
        auto restored=rootMotionPose(inPlace,root,time,displacement(time))*plate->mTransformation;
        double angle=time*3.141592653589793/4,c=std::cos(angle),s=std::sin(angle);
        for(unsigned i=0;i<mesh->mNumVertices;++i) {
            auto v=mesh->mVertices[i],a=original*v,b=restored*v;
            maxError=std::max(maxError,double((a-b).Length()));
            aiVector3D analytic(float(.01+.05*time+(v.x+.005)*c+v.z*s),float(.03+.005*time+v.y),float(-.02-.025*time-(v.x+.005)*s+v.z*c));
            maxAnalyticError=std::max(maxAnalyticError,double((a-analytic).Length()));
            maxSuppressedError=std::max(maxSuppressedError,double((a-stationary*plate->mTransformation*v).Length()));
            require((worldMatrix(other)*v-(v+aiVector3D(.2f,0,0))).Length()<1e-7,"unrelated instance moved");
        }
    }
    for(double time:{-4.25,-2.,-.25,0.,1.999,2.,2.001,4.75}) {
        double cycles=std::floor(time/2.),phase=time-cycles*2.;
        auto accumulated=displacement(phase)+cycle*float(cycles);
        auto a=rootMotionPose(inPlace,root,phase,accumulated)*plate->mTransformation;
        auto b=rootMotionPose(source,root,phase,cycle*float(cycles))*plate->mTransformation;
        for(unsigned i=0;i<mesh->mNumVertices;++i)maxError=std::max(maxError,double((a*mesh->mVertices[i]-b*mesh->mVertices[i]).Length()));
    }
    require(maxError<1e-7 && maxAnalyticError<1e-7,"root-motion recomposition differs from original/analytic animation");
    require(maxSuppressedError>.1,"root-motion removal negative control is ineffective");
    std::cout<<"Root-motion import passed: 17 poses and 8 forward/reverse loop samples, unrelated instance preserved; maximum recomposition error "<<maxError<<" m, analytic error "<<maxAnalyticError<<" m; omitted-motion control "<<maxSuppressedError<<" m\n";
}
