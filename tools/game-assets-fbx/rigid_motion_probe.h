// Schema-2 rigid motion: independent channel evaluation and analytic turning loops.
static aiVector3D rigidVector(const rapidjson::Value& v) {
    require(v.IsArray() && v.Size()==3,"rigid vector shape changed");
    return aiVector3D(float(v[0].GetDouble()),float(v[1].GetDouble()),float(v[2].GetDouble()));
}
static aiQuaternion rigidQuaternion(const rapidjson::Value& v) {
    require(v.IsArray() && v.Size()==4,"rigid quaternion shape changed");
    aiQuaternion q(float(v[3].GetDouble()),float(v[0].GetDouble()),float(v[1].GetDouble()),float(v[2].GetDouble()));
    q.Normalize();return q;
}
static aiMatrix4x4 rigidTransform(const rapidjson::Value& v) {
    return aiMatrix4x4(aiVector3D(1,1,1),rigidQuaternion(v["rotation"]),rigidVector(v["translation"]));
}
static void verifyRigidMotion(const char* path) {
    std::ifstream input(path,std::ios::binary);std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(input)),{});
    require(bytes.size()>=28,"rigid-motion GLB missing");
    auto word=[&](size_t i){return uint32_t(bytes[i])|(uint32_t(bytes[i+1])<<8)|(uint32_t(bytes[i+2])<<16)|(uint32_t(bytes[i+3])<<24);};
    require(word(0)==0x46546c67 && word(4)==2 && word(8)==bytes.size() && word(16)==0x4e4f534a && word(12)<=bytes.size()-20,"rigid-motion GLB header changed");
    rapidjson::Document doc;doc.Parse(reinterpret_cast<const char*>(bytes.data()+20),word(12));
    require(!doc.HasParseError() && doc.HasMember("extras") && doc["extras"].HasMember("gcad_root_motion"),"rigid metadata missing");
    const auto& motion=doc["extras"]["gcad_root_motion"];
    require(motion["schema"].GetUint()==2 && std::string(motion["mode"].GetString())=="rigid" && motion["rotation_extracted"].GetBool(),"rigid extraction mode changed");
    require(motion["source_animation"].GetUint()==0 && motion["in_place_animation"].GetUint()==1 && motion["node"].GetUint()==0,"rigid motion references changed");
    require(std::string(motion["length_unit"].GetString())=="metre" && std::string(motion["up_axis"].GetString())=="Y" && std::string(motion["handedness"].GetString())=="right","rigid units/basis changed");
    require(std::string(motion["composition"].GetString())=="left-multiply-in-place-root" && std::string(motion["loop_composition"].GetString())=="endpoint-power-times-phase","rigid composition contract changed");
    require(motion["duration_seconds"].GetDouble()==2. && motion["max_accumulated_cycles"].GetUint()==1000000,"rigid duration/limit changed");
    for(unsigned k=0;k<3;++k)require(motion["translation_axes"][k].GetBool(),"rigid extraction omitted an axis");
    aiMatrix4x4 anchor=rigidTransform(motion["anchor"]),inverseAnchor=anchor;inverseAnchor.Inverse();
    aiMatrix4x4 endpoint=rigidTransform(motion["loop_transform"]);
    require((anchor*aiVector3D()-aiVector3D(.01f,.03f,-.02f)).Length()<1e-8,"rigid anchor position changed");
    auto absolute=[&](double time) {
        aiVector3D position;aiQuaternion rotation;bool sawPosition=false,sawRotation=false;
        require(motion["channels"].Size()==2,"rigid channel count changed");
        for(const auto& channel:motion["channels"].GetArray()) {
            require(std::string(channel["interpolation"].GetString())=="linear","rigid fixture interpolation changed");
            const auto& times=channel["times_seconds"];const auto& values=channel["values"];
            require(times.Size()==values.Size() && times.Size()>=2,"rigid keys changed");
            unsigned a=0;while(a+1<times.Size() && times[a+1].GetDouble()<=time)++a;unsigned b=std::min(a+1,times.Size()-1);
            float f=a==b?0:float(std::clamp((time-times[a].GetDouble())/(times[b].GetDouble()-times[a].GetDouble()),0.,1.));
            if(std::string(channel["path"].GetString())=="translation") {
                position=rigidVector(values[a])*(1-f)+rigidVector(values[b])*f;sawPosition=true;
            } else {
                require(std::string(channel["path"].GetString())=="rotation","unexpected rigid channel");
                aiQuaternion::Interpolate(rotation,rigidQuaternion(values[a]),rigidQuaternion(values[b]),f);rotation.Normalize();sawRotation=true;
            }
        }
        require(sawPosition && sawRotation,"rigid absolute channels missing");
        return aiMatrix4x4(aiVector3D(1,1,1),rotation,position);
    };
    Assimp::Importer importer;auto* scene=importer.ReadFile(path,0);
    require(scene && scene->mNumMeshes==1 && scene->mNumAnimations==2,"rigid scene changed");
    auto* root=scene->mRootNode->FindNode("root");auto* plate=scene->mRootNode->FindNode("plate");auto* other=scene->mRootNode->FindNode("unrelated");
    require(root && plate && other && plate->mParent==root && other->mParent!=root && plate->mNumMeshes==1 && other->mNumMeshes==1 && plate->mMeshes[0]==other->mMeshes[0],"rigid hierarchy/sharing changed");
    auto* source=scene->mAnimations[0];auto* inPlace=scene->mAnimations[1];
    require(std::string(source->mName.C_Str())=="travel" && std::string(inPlace->mName.C_Str())=="travel-rigid-in-place","rigid clip names changed");
    auto* mesh=scene->mMeshes[plate->mMeshes[0]];require(mesh->mNumVertices==1240 && mesh->mNumFaces==1276,"rigid CAD geometry changed");
    double maxError=0,maxAnalytic=0,maxWrongOrder=0;
    for(unsigned sample=0;sample<=16;++sample) {
        double time=sample/8.;auto stationary=rootMotionPose(inPlace,root,time);
        auto original=rootMotionPose(source,root,time)*plate->mTransformation;
        auto restored=absolute(time)*inverseAnchor*stationary*plate->mTransformation;
        for(unsigned i=0;i<mesh->mNumVertices;++i) {
            auto v=mesh->mVertices[i];maxError=std::max(maxError,double((original*v-restored*v).Length()));
            require((stationary*v-anchor*v).Length()<1e-7,"rigid in-place root still moves");
            require((worldMatrix(other)*v-(v+aiVector3D(.2f,0,0))).Length()<1e-7,"rigid extraction moved unrelated instance");
            require((endpoint*anchor*v-absolute(2.)*v).Length()<1e-7,"rigid endpoint metadata disagrees with channels");
        }
    }
    for(double time:{-4.25,-2.,-.25,0.,1.999,2.,2.001,4.,4.75,7.5}) {
        int cycles=int(std::floor(time/2.));double phase=time-cycles*2.;
        aiMatrix4x4 repeated,step=endpoint;if(cycles<0)step.Inverse();
        for(int i=0;i<std::abs(cycles);++i)repeated=repeated*step;
        auto phaseMotion=absolute(phase)*inverseAnchor;
        auto local=rootMotionPose(inPlace,root,phase)*plate->mTransformation;
        auto restored=repeated*phaseMotion*local,wrongOrder=phaseMotion*repeated*local;
        // Independent fixture trajectory: each cycle turns 90 degrees about Y.
        auto rotate=[](aiVector3D p,double a){return aiVector3D(float(p.x*std::cos(a)+p.z*std::sin(a)),p.y,float(-p.x*std::sin(a)+p.z*std::cos(a)));};
        aiVector3D position(.01f,.03f,-.02f),travel(.1f,.01f,-.05f);double heading=0;
        if(cycles>=0)for(int i=0;i<cycles;++i){position+=rotate(travel,heading);heading+=3.141592653589793/2;}
        else for(int i=0;i>cycles;--i){heading-=3.141592653589793/2;position-=rotate(travel,heading);}
        position+=rotate(travel*float(phase/2),heading);heading+=phase*3.141592653589793/4;
        for(unsigned i=0;i<mesh->mNumVertices;++i) {
            auto v=mesh->mVertices[i],expected=position+rotate(v+aiVector3D(.005f,0,0),heading);
            maxAnalytic=std::max(maxAnalytic,double((restored*v-expected).Length()));
            maxWrongOrder=std::max(maxWrongOrder,double((restored*v-wrongOrder*v).Length()));
        }
    }
    require(maxError<1e-7 && maxAnalytic<2e-7,"rigid motion recomposition/turning trajectory differs");
    require(maxWrongOrder>.01,"rigid multiplication-order control is ineffective");
    std::cout<<"Rigid root-motion import passed: 17 poses and 10 turning/reverse samples; recomposition error "<<maxError<<" m, analytic loop error "<<maxAnalytic<<" m; wrong-order control "<<maxWrongOrder<<" m\n";
}
