// Independent analytic CAD plate bend/twist and child attachment checks.
struct TwistSurfaceReport { double maximum, p95, rms; size_t samples; };
static TwistSurfaceReport verifyTwistChain(const aiScene *scene, bool sliced=false, bool fine=false) {
    require(scene && scene->mNumAnimations == 1, "chain clip missing");
    auto *plate = scene->mRootNode->FindNode("plate");
    auto *attachment = scene->mRootNode->FindNode("attachment");
    auto *payload = scene->mRootNode->FindNode("payload");
    require(plate && attachment && payload && plate->mNumMeshes == 1, "chain nodes missing");
    auto *mesh = scene->mMeshes[plate->mMeshes[0]];
    require(mesh->mNumBones == 5 && mesh->mNumFaces == (sliced ? (fine ? 18228u : 6104u) : 20416u), "chain geometry changed");
    if(sliced) {
        require(mesh->mNumVertices==(fine ? 10082u : 3836u),"sliced vertex count changed");
        for(unsigned f=0;f<mesh->mNumFaces;++f) {
            const auto &face=mesh->mFaces[f]; require(face.mNumIndices==3,"non-triangle slice");
            unsigned intervals=fine ? 32 : 16;
            for(unsigned plane=0;plane<=intervals;++plane) {
                double y=-.015+plane*.03/intervals;
                bool above=false,below=false;
                for(unsigned k=0;k<3;++k) {
                    double coordinate=-mesh->mVertices[face.mIndices[k]].z;
                    above|=coordinate>y+1e-8; below|=coordinate<y-1e-8;
                }
                require(!(above && below),"triangle crosses a slice plane");
            }
        }
    }
    aiNode *bones[5]; aiMatrix4x4 offsets[5]; bool seen[5] = {};
    std::vector<float> weights[5];
    for (unsigned j=0; j<5; ++j) {
        bones[j] = scene->mRootNode->FindNode(("bone"+std::to_string(j)).c_str());
        require(bones[j], "chain bone missing"); weights[j].resize(mesh->mNumVertices);
    }
    require(attachment->mParent == bones[4] && payload->mParent == attachment, "chain attachment hierarchy changed");
    for (unsigned b=0; b<5; ++b) {
        auto *bone = mesh->mBones[b]; std::string name = bone->mName.C_Str();
        require(name.size()==5 && name.substr(0,4)=="bone" && name[4]>='0' && name[4]<='4', "unexpected chain bone");
        unsigned j = unsigned(name[4]-'0'); require(!seen[j], "duplicate chain bone");
        seen[j]=true; offsets[j]=bone->mOffsetMatrix;
        for (unsigned k=0; k<bone->mNumWeights; ++k) {
            auto w=bone->mWeights[k]; require(w.mVertexId<mesh->mNumVertices && w.mWeight>0, "invalid chain weight");
            weights[j][w.mVertexId]+=w.mWeight;
        }
    }
    auto *clip=scene->mAnimations[0];
    require(std::string(clip->mName.C_Str())=="twist" && clip->mNumChannels==4 && clip->mTicksPerSecond>0 &&
            std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6, "chain clip changed");
    const aiNodeAnim *tracks[5] = {};
    for (unsigned c=0;c<4;++c) {
        auto *track=clip->mChannels[c]; std::string name=track->mNodeName.C_Str();
        require(name.size()==5 && name.substr(0,4)=="bone" && name[4]>='1' && name[4]<='4', "unexpected chain track");
        unsigned j=unsigned(name[4]-'0'); require(!tracks[j] && track->mNumRotationKeys==2, "duplicate chain track");
        require(std::abs(track->mRotationKeys[0].mTime)<1e-9 &&
                std::abs(track->mRotationKeys[1].mTime/clip->mTicksPerSecond-1)<1e-6, "chain key bounds changed");
        tracks[j]=track;
    }
    double maxError=0, maxFrameError=0, minRadialRatio=1;
    std::vector<aiVector3D> posed(mesh->mNumVertices);
    std::vector<double> surfaceErrors;
    surfaceErrors.reserve(size_t(mesh->mNumFaces)*33*13);
    double surfaceSquared=0, restSurfaceError=0;
    for (unsigned sample=0;sample<=32;++sample) {
        double time=sample/32.0;
        aiMatrix4x4 worlds[5], matrices[5];
        auto analytic=[&](const aiVector3D &p, unsigned j) {
            double a=time*j*3.14159265358979323846/8;
            double x=p.x, z=p.y-.0015;
            return aiVector3D(float(x*std::cos(a)+z*std::sin(a)),float(.0015-x*std::sin(a)+z*std::cos(a)),p.z);
        };
        for (unsigned j=0;j<5;++j) {
            aiVector3D scale,translation; aiQuaternion rotation;
            bones[j]->mTransformation.Decompose(scale,rotation,translation);
            if(j) {aiQuaternion::Interpolate(rotation,tracks[j]->mRotationKeys[0].mValue,tracks[j]->mRotationKeys[1].mValue,float(time));rotation.Normalize();}
            worlds[j]=worldMatrix(bones[j]->mParent)*aiMatrix4x4(scale,rotation,translation);
            matrices[j]=worlds[j]*offsets[j];
        }
        for(unsigned i=0;i<mesh->mNumVertices;++i) {
            auto p=mesh->mVertices[i]; double v=std::clamp((-double(p.z)+.015)/.0075,0.0,4.0);
            unsigned lo=std::min(unsigned(std::floor(v)),3u); double t=v-lo;
            aiVector3D actual;
            for(unsigned j=0;j<5;++j) {
                double w=j==lo?1-t:j==lo+1?t:0;
                require(std::abs(weights[j][i]-w)<1e-6,"piecewise weights changed");
                actual+=(matrices[j]*p)*weights[j][i];
            }
            auto expected=analytic(p,lo)*float(1-t)+analytic(p,lo+1)*float(t);
            maxError=std::max(maxError,double((actual-expected).Length()));
            posed[i]=actual;
            double restRadius=std::hypot(double(p.x),double(p.y)-.0015);
            if(restRadius>1e-6)
                minRadialRatio=std::min(minRadialRatio,std::hypot(double(actual.x),double(actual.y)-.0015)/restRadius);
        }
        // Compare the imported rendered triangle with the intended continuous
        // piecewise-weight field at the SAME rest-surface point. This is a
        // correspondence error, not closest-surface distance or a global bound.
        auto continuous=[&](double x, double y, double z) {
            double v=std::clamp((-z+.015)/.0075,0.0,4.0);
            unsigned lo=std::min(unsigned(std::floor(v)),3u);
            double blend=v-lo;
            auto rotate=[&](unsigned joint) {
                double a=time*joint*3.14159265358979323846/8;
                return std::array<double,3>{x*std::cos(a)+(y-.0015)*std::sin(a),
                    .0015-x*std::sin(a)+(y-.0015)*std::cos(a),z};
            };
            auto a=rotate(lo),b=rotate(lo+1);
            return std::array<double,3>{a[0]*(1-blend)+b[0]*blend,a[1]*(1-blend)+b[1]*blend,z};
        };
        for(unsigned f=0;f<mesh->mNumFaces;++f) {
            const auto &face=mesh->mFaces[f]; require(face.mNumIndices==3,"non-triangle twist surface");
            auto samplePoint=[&](double a,double b,double c) {
                double bary[3]={a,b,c},rest[3]={},rendered[3]={};
                for(unsigned corner=0;corner<3;++corner) {
                    auto index=face.mIndices[corner];
                    for(unsigned axis=0;axis<3;++axis) {
                        rest[axis]+=bary[corner]*mesh->mVertices[index][axis];
                        rendered[axis]+=bary[corner]*posed[index][axis];
                    }
                }
                auto expected=continuous(rest[0],rest[1],rest[2]);
                double squared=0;
                for(unsigned axis=0;axis<3;++axis) squared+=(rendered[axis]-expected[axis])*(rendered[axis]-expected[axis]);
                double error=std::sqrt(squared);
                require(std::isfinite(error),"nonfinite twist surface error");
                surfaceErrors.push_back(error); surfaceSquared+=squared;
                if(sample==0) restSurfaceError=std::max(restSurfaceError,error);
            };
            // 12 non-vertex points on a denominator-four barycentric grid,
            // plus the centroid. Shared edge samples count once per face.
            for(unsigned a=0;a<=4;++a) for(unsigned b=0;b<=4-a;++b) {
                unsigned c=4-a-b;
                if(a==4 || b==4 || c==4) continue;
                samplePoint(a/4.0,b/4.0,c/4.0);
            }
            samplePoint(1.0/3,1.0/3,1.0/3);
        }
        auto frame=worlds[4]*attachment->mTransformation*payload->mTransformation;
        for(unsigned k=0;k<4;++k) {
            aiVector3D point(k==1?.001f:0,k==2?.001f:0,k==3?.001f:0);
            auto expected=analytic(point+aiVector3D(.01f,.0015f,-.005f),4);
            maxFrameError=std::max(maxFrameError,double((frame*point-expected).Length()));
        }
    }
    require(maxError<1e-6 && maxFrameError<1e-6,"chain analytic comparison failed");
    require(minRadialRatio>std::cos(3.14159265358979323846/16)-1e-5,"chain radial shrink exceeds analytic bound");
    require(restSurfaceError<1e-7,"rest surface calibration failed");
    std::sort(surfaceErrors.begin(),surfaceErrors.end());
    TwistSurfaceReport surface{surfaceErrors.back(),surfaceErrors[size_t(std::ceil(.95*surfaceErrors.size()))-1],
        std::sqrt(surfaceSquared/surfaceErrors.size()),surfaceErrors.size()};
    require(surface.maximum>maxError*1000,"surface check failed to detect non-vertex deformation error");
    std::cout<<"Twist chain import passed: five bones, 33 poses; skin error "<<maxError<<" m, attachment frame error "<<maxFrameError<<" m; minimum vertex radial ratio "<<minRadialRatio<<"\n";
    std::cout<<"Twist surface correspondence: "<<surface.samples<<" samples; maximum "<<surface.maximum*1000
             <<" mm, p95 "<<surface.p95*1000<<" mm, RMS "<<surface.rms*1000<<" mm; rest calibration "<<restSurfaceError<<" m\n";
    return surface;
}

static void verifyAttachments(const aiScene *scene) {
    require(scene && scene->mNumAnimations == 2, "attachment fixture needs two clips");
    auto *plate = scene->mRootNode->FindNode("plate");
    auto *base = scene->mRootNode->FindNode("base");
    auto *tip = scene->mRootNode->FindNode("tip");
    auto *attachment = scene->mRootNode->FindNode("attachment");
    auto *payload = scene->mRootNode->FindNode("payload");
    require(plate && base && tip && attachment && payload, "attachment nodes lost");
    require(tip->mParent == base && attachment->mParent == tip && payload->mParent == attachment,
            "attachment hierarchy changed");
    require(plate->mNumMeshes == 1 && attachment->mNumMeshes == 0 && payload->mNumMeshes == 0,
            "attachment fixture mesh assignment changed");
    auto *mesh = scene->mMeshes[plate->mMeshes[0]];
    require(mesh->mNumBones == 2 && mesh->mNumVertices > 10000 && mesh->mNumFaces == 20416,
            "refined plate rig missing");
    std::vector<float> weights[2];
    for (auto &w : weights) w.resize(mesh->mNumVertices);
    aiMatrix4x4 offsets[2];
    bool seen[2] = {};
    for (unsigned b = 0; b < mesh->mNumBones; ++b) {
        auto *bone = mesh->mBones[b];
        std::string name = bone->mName.C_Str();
        require(name == "base" || name == "tip", "unexpected attachment bone");
        unsigned j = name == "base" ? 0 : 1;
        require(!seen[j], "duplicate attachment bone");
        seen[j] = true; offsets[j] = bone->mOffsetMatrix;
        for (unsigned k = 0; k < bone->mNumWeights; ++k) {
            auto w = bone->mWeights[k];
            require(w.mVertexId < mesh->mNumVertices && w.mWeight > 0, "invalid influence");
            weights[j][w.mVertexId] += w.mWeight;
        }
    }
    double maxVertexError = 0, maxAttachmentError = 0, omittedRotationError = 0;
    bool bendSeen = false, twistSeen = false;
    for (unsigned c = 0; c < scene->mNumAnimations; ++c) {
        auto *clip = scene->mAnimations[c];
        std::string name = clip->mName.C_Str();
        bool twist = name == "twist";
        require(twist || name == "bend", "unexpected clip");
        require(twist ? !twistSeen : !bendSeen, "duplicate clip");
        (twist ? twistSeen : bendSeen) = true;
        require(clip->mNumChannels == 1 && clip->mTicksPerSecond > 0 &&
                std::abs(clip->mDuration / clip->mTicksPerSecond - 1) < 1e-6, "clip timing changed");
        auto *track = clip->mChannels[0];
        require(std::string(track->mNodeName.C_Str()) == "tip" && track->mNumRotationKeys == 2,
                "attachment rotation track changed");
        require(std::abs(track->mRotationKeys[0].mTime) < 1e-9 &&
                std::abs(track->mRotationKeys[1].mTime / clip->mTicksPerSecond - 1) < 1e-6,
                "attachment key bounds changed");
        for (unsigned sample = 0; sample <= 32; ++sample) {
            double t = sample / 32.0, angle = t * 3.14159265358979323846 / 2;
            // Analytic rotation about CAD Y for twist, CAD X for bend, then basis conversion.
            auto rotate = [&](double x, double y, double z) {
                double rx = twist ? x * std::cos(angle) + z * std::sin(angle) : x;
                double ry = twist ? y : y * std::cos(angle) - z * std::sin(angle);
                double rz = twist ? -x * std::sin(angle) + z * std::cos(angle) : y * std::sin(angle) + z * std::cos(angle);
                return aiVector3D(float(rx), float(rz), float(-ry));
            };
            aiVector3D scale, translation; aiQuaternion rotation;
            tip->mTransformation.Decompose(scale, rotation, translation);
            aiQuaternion::Interpolate(rotation, track->mRotationKeys[0].mValue,
                                      track->mRotationKeys[1].mValue, float(t));
            rotation.Normalize();
            auto tipWorld = worldMatrix(base) * aiMatrix4x4(scale, rotation, translation);
            aiMatrix4x4 matrices[2] = {worldMatrix(base) * offsets[0], tipWorld * offsets[1]};
            for (unsigned i = 0; i < mesh->mNumVertices; ++i) {
                auto p = mesh->mVertices[i];
                double w = std::clamp((-double(p.z) + .015) / .03, 0.0, 1.0);
                require(std::abs(weights[1][i] - w) < 1e-6 && std::abs(weights[0][i] - (1-w)) < 1e-6,
                        "attachment blend weights changed");
                auto expected = p * float(1-w) + (rotate(p.x, -p.z, p.y-.0015) + aiVector3D(0,.0015f,0)) * float(w);
                auto actual = (matrices[0]*p)*weights[0][i] + (matrices[1]*p)*weights[1][i];
                maxVertexError = std::max(maxVertexError, double((actual-expected).Length()));
            }
            auto attachmentWorld = tipWorld * attachment->mTransformation;
            auto payloadWorld = attachmentWorld * payload->mTransformation;
            // Origin plus three basis endpoints verifies orientation as well as translation.
            for (unsigned point = 0; point < 4; ++point) {
                aiVector3D local(point==1?.001f:0, point==2?.001f:0, point==3?.001f:0);
                for (unsigned child = 0; child < 2; ++child) {
                    auto expected = rotate(.01+local.x, (child?.005:0)-local.z, local.y) + aiVector3D(0,.0015f,0);
                    auto actual = (child ? payloadWorld : attachmentWorld) * local;
                    maxAttachmentError = std::max(maxAttachmentError, double((actual-expected).Length()));
                    if (child && point==0)
                        omittedRotationError = std::max(omittedRotationError, double((expected-aiVector3D(.01f,.0015f,-.005f)).Length()));
                }
            }
        }
    }
    require(bendSeen && twistSeen && maxVertexError < 1e-6 && maxAttachmentError < 1e-6,
            "analytic skin or attachment comparison failed");
    require(omittedRotationError > .01, "attachment negative control insensitive");
    std::cout << "Attachment/twist import passed: 66 poses, " << mesh->mNumVertices
              << " vertices; max skin error " << maxVertexError << " m, attachment frame error "
              << maxAttachmentError << " m; omitted-rotation control " << omittedRotationError << " m\n";
}
