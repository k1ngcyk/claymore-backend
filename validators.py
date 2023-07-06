# Pydantic
from datetime import datetime
from typing import List, Optional, Union
from pydantic import BaseModel
from enum import Enum

class Status(str, Enum):
    success = 'Success'
    failure = 'Failure'

class CandidateStatus(str, Enum):
    testing = 'Testing'
    candidate = 'Candidate'
    canon = 'Canon'
    removed = 'Removed'

class GenerationJobActionType(str, Enum):
    start = 'Start'
    stop = 'Stop'
    retry = 'Retry'
    create = 'Create'

class PostResponse(BaseModel):
    status: Status
    message: str

class Candidate(BaseModel):
    content: str
    status: CandidateStatus

class CandidateList(BaseModel):
    candidates: List[Candidate]
    
class DialogCandidateResponse(BaseModel):
    candidates: List[Candidate]

class FilterValue(BaseModel):
    field: str
    type: 'value'
    value: str

class FilterExists(BaseModel):
    field: str
    type: 'exists'
    value: bool

Filter = Union[FilterValue, FilterExists]

class GenerationJob(BaseModel):
    id: int
    name: str
    created_at: datetime
    status: str
    progress: float

class JobConfig(BaseModel):
    model: str
    temperature: str
    length: str

class JobDetailResponse(BaseModel):
    progress: float
    token: int
    duration: datetime
    generator: List[str]
    config: JobConfig
    feedback: int


class Setting(BaseModel):
    key: str
    value: str

class LoginRequest(BaseModel):
    name: str
    password: str

class RegisterRequest(BaseModel):
    name: str
    password: str

class AddGeneratorRequest(BaseModel):
    name: str
    content: List[str]

class CreateGenerationJobRequest(BaseModel):
    generator_id: int
    count: int

class GenerationJobActionRequest(BaseModel):
    type: GenerationJobActionType

class GetAllGenerationJobsRequest(BaseModel):
    filter: str

class GetDialogRequest(BaseModel):
    filter: Filter

class EditDialogRequest(BaseModel):
    field: str
    content: str

class AddSettingRequest(BaseModel):
    key: str
    value: str

class EditSettingRequest(BaseModel):
    type: str
    field: str
    content: str

class DeleteSettingRequest(BaseModel):
    type: str

class GetSettingRequest(BaseModel):
    type: str