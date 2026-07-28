Rails.application.routes.draw do
  devise_for :users, skip: [:registrations]

  resources :projects do
    resources :documents, only: [:create, :update, :destroy], controller: "project_documents" do
      member do
        get :viewer
        post :save_extraction
        post :upload_export
      end
    end
    resources :document_extractions, only: [:destroy]

    member do
      get :confirm_destroy
    end
  end

  resources :client_submissions do
    resources :documents, only: [:create, :update, :destroy], controller: "client_submission_documents"

    member do
      post :submit
      post :convert
      post :attach_to_project
      post :archive
      post :unarchive
    end
  end

  resources :contributors do
    resource :portal_access, only: [:create, :update, :destroy], controller: "contributor_portal_access"

    member do
      get :confirm_destroy
    end
  end

  resources :contributor_types do
    member do
      get :confirm_destroy
    end
  end

  root "projects#index"
end
