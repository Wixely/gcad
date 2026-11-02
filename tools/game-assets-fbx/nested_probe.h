// Analytic oracle for examples/game-assets/nested-assembly.gasset; no gcad math.
static aiMatrix4x4 nestedPose(const aiNode* node,const aiAnimation* clip,double seconds) {
    const aiNodeAnim* channel=nullptr;
    for(unsigned i=0;i<clip->mNumChannels;++i)if(clip->mChannels[i]->mNodeName==node->mName)channel=clip->mChannels[i];
    if(!channel)return node->mTransformation;
    double ticks=seconds*clip->mTicksPerSecond;
    auto vector=[&](const aiVectorKey* keys,unsigned count){
        require(count>0,"nested vector keys missing");unsigned a=0;
        while(a+1<count && keys[a+1].mTime<=ticks)++a;
        unsigned b=std::min(a+1,count-1);float f=a==b?0:float((ticks-keys[a].mTime)/(keys[b].mTime-keys[a].mTime));
        return keys[a].mValue*(1-f)+keys[b].mValue*f;
    };
    unsigned count=channel->mNumRotationKeys;require(count>0,"nested rotation keys missing");unsigned a=0;
    while(a+1<count && channel->mRotationKeys[a+1].mTime<=ticks)++a;
    unsigned b=std::min(a+1,count-1);float f=a==b?0:float((ticks-channel->mRotationKeys[a].mTime)/(channel->mRotationKeys[b].mTime-channel->mRotationKeys[a].mTime));
    aiQuaternion q;aiQuaternion::Interpolate(q,channel->mRotationKeys[a].mValue,channel->mRotationKeys[b].mValue,f);q.Normalize();
    return aiMatrix4x4(vector(channel->mScalingKeys,channel->mNumScalingKeys),q,vector(channel->mPositionKeys,channel->mNumPositionKeys));
}

static double checkNested(const aiScene* scene) {
    require(scene && scene->mNumAnimations==1,"nested clip missing");const auto* clip=scene->mAnimations[0];
    require(clip->mNumChannels==4 && clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"nested clip shape changed");
    for(unsigned c=0;c<clip->mNumChannels;++c){auto* channel=clip->mChannels[c];std::string name=channel->mNodeName.C_Str();
        require(name=="first/left.a" || name=="first/left.b" || name=="first/right.a" || name=="first/right.b","nested channel target changed");
        require(channel->mNumPositionKeys==25 && channel->mNumRotationKeys==25 && channel->mNumScalingKeys==25,"nested key count changed");}
    auto find=[&](const std::string& name){auto* n=scene->mRootNode->FindNode(name.c_str());require(n!=nullptr,"nested node missing");return n;};
    double maximum=0;const double pi=3.14159265358979323846;
    for(unsigned copy=0;copy<2;++copy)for(unsigned side=0;side<2;++side)for(unsigned part=0;part<2;++part){
        std::string owner=copy?"second":"first", branch=side?"right":"left", body=branch+(part?".b":".a");
        auto* root=find(owner);auto* group=find(owner+"/"+branch+"/");auto* inner=find(owner+"/"+branch+"/a/");auto* node=find(owner+"/"+body);
        require(group->mParent==root && inner->mParent==group && node->mParent==(part?group:inner),"nested parent changed");
        auto* paired=find((copy?"first/":"second/")+body);
        require(node->mNumMeshes==1 && paired->mNumMeshes==1 && node->mMeshes[0]==paired->mMeshes[0] && node->mMeshes[0]<scene->mNumMeshes,"nested mesh sharing changed");
        auto* mesh=scene->mMeshes[node->mMeshes[0]];
        require(mesh->mNumFaces==12 && mesh->HasNormals() && mesh->HasTextureCoords(0),"nested block geometry changed");
        for(unsigned sample=0;sample<=96;++sample){double time=sample/96.;aiMatrix4x4 world;
            for(auto* current=node;current;current=current->mParent)world=nestedPose(current,clip,time)*world;
            for(unsigned v=0;v<mesh->mNumVertices;++v){auto p=mesh->mVertices[v];double x=p.x*1000,y=-p.z*1000,z=p.y*1000;
                require(std::abs(std::abs(x)-1)<1e-4 && std::abs(std::abs(y)-1.5)<1e-4 && (std::abs(z)<1e-4 || std::abs(z-4)<1e-4),"nested local geometry moved");
                if(part)x+=8;else{x+=1;y+=2;z+=3;}
                if(!copy && !side && !part){double a=time*pi/6,old=x;x=old*std::cos(a)-y*std::sin(a);y=old*std::sin(a)+y*std::cos(a);}
                if(side)x-=20;else{
                    x+=10;double a=37*pi/180,c=std::cos(a),s=std::sin(a),d=std::sqrt(14.),u=1/d,w=3/d,q=2/d;
                    double dot=u*x+q*y+w*z,xx=x*c+(q*z-w*y)*s+u*dot*(1-c),yy=y*c+(w*x-u*z)*s+q*dot*(1-c);
                    z=z*c+(u*y-q*x)*s+w*dot*(1-c);x=xx;y=yy;
                }
                if(copy){x=100-1.25*x;y*=.75;z*=1.5;}
                aiVector3D expected(float(x/1000),float(z/1000),float(-y/1000));
                double error=(world*p-expected).Length();maximum=std::max(maximum,error);
                require(error<(sample%4==0?1e-7:1e-6),"nested animated world position differs");
            }
        }
    }
    return maximum;
}

static void verifyNested(const char* path) {
    Assimp::Importer importer;const auto* scene=importer.ReadFile(path,0);double maximum=checkNested(scene);
    auto rejects=[&](){bool failed=false;try{checkNested(scene);}catch(const std::exception&){failed=true;}require(failed,"nested corruption escaped");};
    auto* group=scene->mRootNode->FindNode("first/left/");auto saved=group->mTransformation;group->mTransformation.a4+=.02f;rejects();group->mTransformation=saved;
    auto* body=scene->mRootNode->FindNode("first/left.a");auto* parent=body->mParent;body->mParent=group;rejects();body->mParent=parent;
    auto* mirror=scene->mRootNode->FindNode("second");saved=mirror->mTransformation;mirror->mTransformation.a1*=-1;rejects();mirror->mTransformation=saved;
    auto* channel=scene->mAnimations[0]->mChannels[0];auto position=channel->mPositionKeys[0].mValue;channel->mPositionKeys[0].mValue.x+=.02f;rejects();channel->mPositionKeys[0].mValue=position;
    checkNested(scene);
    std::cout<<"Nested assembly import passed: 8 shared body instances, 97 poses, analytic world positions; maximum error "<<maximum<<" m; 4 hierarchy/placement/reflection/key controls rejected\n";
}

static double checkNestedSkin(const aiScene* scene) {
    require(scene && scene->mNumAnimations==1,"nested skin clip missing");
    auto* node=scene->mRootNode->FindNode("first/left.a");auto* copy=scene->mRootNode->FindNode("second/left.a");auto* joint=scene->mRootNode->FindNode("bone");
    require(node && copy && joint && node->mNumMeshes==1 && copy->mNumMeshes==1,"nested skin nodes missing");
    require(std::string(node->mParent->mName.C_Str())=="first/left/a/" && std::string(node->mParent->mParent->mName.C_Str())=="first/left/","nested skin hierarchy changed");
    auto* mesh=scene->mMeshes[node->mMeshes[0]];auto* rigid=scene->mMeshes[copy->mMeshes[0]];
    require(mesh!=rigid && mesh->mNumBones==1 && rigid->mNumBones==0 && mesh->mNumVertices==rigid->mNumVertices,"nested skin leaked to shared instance");
    auto* bone=mesh->mBones[0];require(std::string(bone->mName.C_Str())=="bone" && bone->mNumWeights==mesh->mNumVertices,"nested bone weights missing");
    std::vector<bool> weighted(mesh->mNumVertices,false);
    for(unsigned i=0;i<bone->mNumWeights;++i){auto weight=bone->mWeights[i];require(weight.mVertexId<weighted.size() && !weighted[weight.mVertexId] && std::abs(weight.mWeight-1)<1e-6,"nested rigid weight changed");weighted[weight.mVertexId]=true;}
    auto* clip=scene->mAnimations[0];require(clip->mNumChannels==1 && clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"nested lift clip changed");
    auto* channel=clip->mChannels[0];require(std::string(channel->mNodeName.C_Str())=="bone" && channel->mNumPositionKeys==2,"nested lift target changed");
    aiVector3D scale,position;aiQuaternion rotation;joint->mTransformation.Decompose(scale,rotation,position);
    auto expected=[&](aiVector3D p,bool mirrored,double time){
        double x=p.x*1000+11,y=-p.z*1000+2,z=p.y*1000+3;
        double angle=37*3.14159265358979323846/180,c=std::cos(angle),s=std::sin(angle),d=std::sqrt(14.),u=1/d,v=2/d,w=3/d;
        double dot=u*x+v*y+w*z,xx=x*c+(v*z-w*y)*s+u*dot*(1-c),yy=y*c+(w*x-u*z)*s+v*dot*(1-c);
        z=z*c+(u*y-v*x)*s+w*dot*(1-c);x=xx;y=yy;
        if(mirrored){x=100-1.25*x;y*=.75;z*=1.5;}else z+=10*time;
        return aiVector3D(float(x/1000),float(z/1000),float(-y/1000));
    };
    double maximum=0;
    for(unsigned sample=0;sample<=32;++sample){float t=sample/32.f;
        position=channel->mPositionKeys[0].mValue*(1-t)+channel->mPositionKeys[1].mValue*t;
        auto pose=worldMatrix(joint->mParent)*aiMatrix4x4(scale,rotation,position)*bone->mOffsetMatrix;
        for(unsigned v=0;v<mesh->mNumVertices;++v){
            require((mesh->mVertices[v]-rigid->mVertices[v]).Length()<1e-7,"nested source tuples differ");
            auto p=mesh->mVertices[v];require(std::abs(std::abs(p.x)-.001)<1e-7 && std::abs(std::abs(p.z)-.0015)<1e-7 && (std::abs(p.y)<1e-7 || std::abs(p.y-.004)<1e-7),"nested skin local geometry changed");
            double error=(pose*p-expected(p,false,t)).Length();maximum=std::max(maximum,error);
            require(error<1e-7,"nested skin world pose differs");
            require((worldMatrix(copy)*rigid->mVertices[v]-expected(p,true,0)).Length()<1e-7,"nested rigid copy differs");
        }
    }
    return maximum;
}

static void verifyNestedSkin(const char* path) {
    Assimp::Importer importer;const auto* scene=importer.ReadFile(path,0);double maximum=checkNestedSkin(scene);
    auto rejects=[&](){bool failed=false;try{checkNestedSkin(scene);}catch(const std::exception&){failed=true;}require(failed,"nested skin corruption escaped");};
    auto* node=scene->mRootNode->FindNode("first/left.a");auto* bone=scene->mMeshes[node->mMeshes[0]]->mBones[0];
    auto weight=bone->mWeights[0].mWeight;bone->mWeights[0].mWeight=.5f;rejects();bone->mWeights[0].mWeight=weight;
    auto offset=bone->mOffsetMatrix;bone->mOffsetMatrix.a4+=.02f;rejects();bone->mOffsetMatrix=offset;
    auto* parent=node->mParent;node->mParent=parent->mParent;rejects();node->mParent=parent;
    auto* channel=scene->mAnimations[0]->mChannels[0];auto position=channel->mPositionKeys[1].mValue;channel->mPositionKeys[1].mValue.y+=.02f;rejects();channel->mPositionKeys[1].mValue=position;
    checkNestedSkin(scene);
    std::cout<<"Nested skin import passed: isolated rigid binding, nonidentity inverse bind, 33 analytic world poses; maximum error "<<maximum<<" m; 4 weight/bind/parent/key controls rejected\n";
}
