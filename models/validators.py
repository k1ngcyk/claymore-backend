# Pydantic
from datetime import datetime, time
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
    start = 'start'
    stop = 'stop'
    retry = 'retry'


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


class GenerationJobResponse(BaseModel):
    id: int
    name: str
    created_at: datetime
    status: str
    progress: float


class GenerationJobListResponse(BaseModel):
    jobs: List[GenerationJobResponse]


class JobConfig(BaseModel):
    model: str
    temperature: float
    length: float


class JobDetailResponse(BaseModel):
    progress: float
    token: int
    duration: int
    generator: List[str]
    config: JobConfig


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
    user_id: int


class CreateGenerationJobRequest(BaseModel):
    generator_id: int
    count: int


class GenerationJobActionRequest(BaseModel):
    type: GenerationJobActionType


class GetAllGenerationJobsRequest(BaseModel):
    filter: str


class EditDialogRequest(BaseModel):
    field: str
    content: str
    user_id: int


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
